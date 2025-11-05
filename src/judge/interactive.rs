use std::sync::Arc;

use channel::Channel;
use tokio::{fs::File, io::AsyncReadExt as _};

use super::{Lang, ProblemLimits, SOLUTION_EXT, SOLUTION_NAME, TestResult, path_from};
use crate::{
    LogState, Result,
    judge::Verdict,
    sandbox::{self, MaybeLimited, RunConfig, RunStatus},
};

pub const TEST_DIR: &str = "test";
pub const TEST_EXT: Option<&str> = Some("txt");

pub const INTERACTOR_NAME: &str = "interactor";
pub const INTERACTOR_EXT: Option<&str> = Some("out");

pub const CHANNEL_DIR: &str = "/.invoker";

pub struct Enviroment {
    sandbox: Arc<sandbox::Sandbox>,
    interactor_sandbox: Arc<sandbox::Sandbox>,
    limits: ProblemLimits,
    lang: Lang,

    work_dir: Box<str>,
    test_id: usize,
    log_state: Arc<LogState>,
}

pub async fn prepare(
    sandboxes: Arc<sandbox::Service>,
    lang: Lang,
    limits: ProblemLimits,
    work_dir: Box<str>,

    test_id: usize,
    log_state: Arc<LogState>,
) -> Result<Enviroment> {
    let sandbox = Arc::new(Arc::clone(&sandboxes).initialize_sandbox().await?);
    let interactor_sandbox = Arc::new(sandboxes.initialize_sandbox().await?);

    let log_state = log_state.push("solution_box_id", &*format!("{}", sandbox.id()));
    let log_state = log_state.push(
        "interactor_box_id",
        &*format!("{}", interactor_sandbox.id()),
    );

    Ok(Enviroment {
        sandbox,
        interactor_sandbox,
        lang,
        limits,
        work_dir,
        test_id,
        log_state,
    })
}

impl super::Enviroment for Enviroment {
    fn run<'a>(
        self: Box<Self>,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<TestResult>> + Send + 'a>> {
        Box::pin(async move {
            let log_state = self.log_state.push("task type", "INTERACTIVE");

            let src_test_path = path_from(
                &format!("{}/{}", self.work_dir, TEST_DIR),
                &format!("{}", self.test_id + 1),
                TEST_EXT,
            );

            let src_interactor_path = path_from(&self.work_dir, INTERACTOR_NAME, INTERACTOR_EXT);
            let src_solution_path = path_from(&self.work_dir, SOLUTION_NAME, SOLUTION_EXT);

            const TARGET_TEST_PATH: &str = "test.txt";
            const TARGET_INTERACTOR_OUTPUT_PATH: &str = "interactor_out.txt";
            const TARGET_INTERACTOR_ERROR_PATH: &str = "interactor_err.txt";

            const TARGET_INTERACTOR_PATH: &str = "interactor.out";
            const TARGET_SOLUTION_PATH: &str = "solution.out";

            Arc::clone(&self.interactor_sandbox)
                .write_group_into_box(
                    vec![
                        (File::open(&*src_test_path).await?, TARGET_TEST_PATH),
                        (
                            File::open(&*src_interactor_path).await?,
                            TARGET_INTERACTOR_PATH,
                        ),
                    ]
                    .into_iter()
                    .map(|(from, to)| (from, Box::from(to)))
                    .collect(),
                )
                .await?;
            self.sandbox
                .write_into_box(
                    &mut File::open(&*src_solution_path).await?,
                    TARGET_SOLUTION_PATH,
                )
                .await?;

            let solution_input_channel = Channel::new(CHANNEL_DIR).await?;
            let solution_output_channel = Channel::new(CHANNEL_DIR).await?;

            let _solution_output_keeper = File::options()
                .read(true)
                .write(true)
                .open(&*solution_output_channel.0)
                .await?;

            let _solution_input_keeper = File::options()
                .read(true)
                .write(true)
                .open(&*solution_input_channel.0)
                .await?;

            let interactor_sandbox_clone = Arc::clone(&self.interactor_sandbox);
            let interactor_run_config = RunConfig {
                time_limit: MaybeLimited::Limited(self.limits.time),
                memory_limit: MaybeLimited::Unlimited,
                real_time_limit: self.limits.real_time,
                extra_time_limit: None,
                stack_limit: Some(MaybeLimited::Unlimited),
                open_files_limit: None,
                process_limit: Some(MaybeLimited::Unlimited),
                open_dirs: Box::from(vec![Box::from(CHANNEL_DIR)]),

                env: false,

                stdin: Some(solution_output_channel.0.clone()),
                stdout: Some(solution_input_channel.0.clone()),
                stderr: Some(TARGET_INTERACTOR_ERROR_PATH.to_string().into_boxed_str()),
            };
            let lang = self.lang;
            let interactor_handler = tokio::spawn(async move {
                interactor_sandbox_clone.run(
                    lang.run_command(&*format!("./{TARGET_INTERACTOR_PATH} {TARGET_TEST_PATH} {TARGET_INTERACTOR_OUTPUT_PATH}")
                        .into_boxed_str(),),
                    interactor_run_config,
                ).await
            });

            let sandbox_clone = Arc::clone(&self.sandbox);

            let solution_run_config = RunConfig {
                time_limit: MaybeLimited::Limited(self.limits.time),
                memory_limit: MaybeLimited::Limited(self.limits.memory),
                real_time_limit: self.limits.real_time,
                extra_time_limit: None,
                stack_limit: self.limits.stack.map(|s| MaybeLimited::Limited(s)),
                open_files_limit: None,
                process_limit: Some(MaybeLimited::Limited(1)),
                env: false,
                open_dirs: Box::from(vec![Box::from(CHANNEL_DIR)]),

                stdin: Some(solution_input_channel.0.clone()),
                stdout: Some(solution_output_channel.0.clone()),
                stderr: None,
            };

            let solution_handler = tokio::spawn(async move {
                sandbox_clone
                    .run(lang.run_command(TARGET_SOLUTION_PATH), solution_run_config)
                    .await
            });

            let solution_result = match solution_handler.await? {
                Ok(res) => res,
                Err(e) => {
                    log::error!("({log_state}) solution run error: {e:?}");
                    return Err(e);
                }
            };

            let interactor_result = match interactor_handler.await? {
                Ok(res) => res,
                Err(e) => {
                    log::error!("({log_state}) interactor run error: {e:?}");
                    return Err(e);
                }
            };

            log::trace!("({log_state}) starting reading");

            let interactor_output: Arc<str> = Arc::from(&*if let Ok(mut file) = self
                .interactor_sandbox
                .read_from_box(TARGET_INTERACTOR_OUTPUT_PATH)
                .await
            {
                let mut output_error = String::new();
                file.read_to_string(&mut output_error).await?;
                output_error
            } else {
                String::new()
            });

            let interactor_error: Arc<str> = Arc::from(&*if let Ok(mut file) = self
                .interactor_sandbox
                .read_from_box(TARGET_INTERACTOR_ERROR_PATH)
                .await
            {
                let mut interactor_error = String::new();
                file.read_to_string(&mut interactor_error).await?;
                interactor_error
            } else {
                String::new()
            });

            if let Some(verdict) = Verdict::from_run_status(solution_result.status) {
                return Ok(TestResult {
                    verdict,
                    time: solution_result.time,
                    memory: solution_result.memory,
                    output: interactor_output,
                    message: Arc::from(
                        format!(
                            "ISOLATE: {}\nINTERACTOR_ERRORS: {}",
                            solution_result.status_message.unwrap_or(Box::from("-")),
                            interactor_error,
                        )
                        .as_str(),
                    ),
                });
            }

            let (verdict, message) = match interactor_result.status {
                RunStatus::Ml | RunStatus::Tl | RunStatus::Sg(_) => (
                    Verdict::Te,
                    format!(
                        "interactor_output: {interactor_output}\n, interactor_error: {interactor_error}\n 'isolate': {}",
                        interactor_result.status_message.as_deref().unwrap_or("")
                    ),
                ),
                RunStatus::Ok => (
                    Verdict::Ok,
                    format!(
                        "interactor_output: {interactor_output}\n, interactor_error: {interactor_error}\n 'isolate': {}",
                        interactor_result.status_message.as_deref().unwrap_or("")
                    ),
                ),
                RunStatus::Re(code) => (
                    match code {
                        1 => Verdict::Wa,
                        2 => Verdict::Pe,
                        _ => Verdict::Te,
                    },
                    format!(
                        "interactor_output: {interactor_output}\n, interactor_error: {interactor_error}"
                    ),
                ),
            };

            Ok(TestResult {
                verdict,
                message: Arc::from(message),

                output: interactor_output,
                memory: solution_result.memory,
                time: solution_result.time,
            })
        })
    }
}
