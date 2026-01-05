use std::sync::Arc;

use async_trait::async_trait;
use tokio::{fs::File, io::AsyncReadExt as _, task::JoinHandle};

use super::{
    Lang, SOLUTION_EXT, SOLUTION_NAME,
    api::{submission, test},
    path_from,
};
use crate::{
    LogState, Result,
    sandbox::{self, Command, MaybeLimited::*, RunStatus},
};

const CHECKER_NAME: &str = "checker";
const CHECKER_EXT: Option<&str> = Some("out");

const INPUT_DIR: &str = "input";
const INPUT_EXT: Option<&str> = Some("txt");

const CORRECT_DIR: &str = "correct";
const CORRECT_EXT: Option<&str> = Some("txt");

pub struct Enviroment {
    sandbox: Arc<sandbox::Sandbox>,
    limits: submission::Limits,
    lang: Lang,

    work_dir: Box<str>,
    test_id: usize,
    log_state: Arc<LogState>,
}

pub async fn prepare(
    sandboxes: Arc<sandbox::Service>,
    lang: Lang,
    limits: submission::Limits,
    work_dir: Box<str>,

    test_id: usize,
    log_state: Arc<LogState>,
) -> Result<Enviroment> {
    let sandbox = Arc::new(sandboxes.initialize_sandbox().await?);

    let log_state = log_state.push("box_id", &*format!("{}", sandbox.id()));

    Ok(Enviroment {
        sandbox,
        lang,
        limits,
        work_dir,
        test_id,
        log_state,
    })
}

#[async_trait]
impl super::Enviroment for Enviroment {
    async fn run(self: Box<Self>) -> Result<test::Result> {
        let log_state = self.log_state.push("task type", "STANDARD");
        log::trace!("({log_state}) testing STARTED");
        let src_input_path = super::path_from(
            &format!("{}/{INPUT_DIR}", self.work_dir),
            &format!("{}", self.test_id + 1),
            INPUT_EXT,
        );
        let src_correct_path = path_from(
            &format!("{}/{CORRECT_DIR}", self.work_dir),
            &format!("{}", self.test_id + 1),
            CORRECT_EXT,
        );
        let src_checker_path = path_from(&self.work_dir, CHECKER_NAME, CHECKER_EXT);

        let src_solution_path = path_from(&self.work_dir, SOLUTION_NAME, SOLUTION_EXT);

        const TARGET_INPUT_PATH: &str = "in.txt";
        const TARGET_CORRECT_PATH: &str = "correct.txt";
        const TARGET_OUTPUT_PATH: &str = "out.txt";
        const TARGET_CHECKER_OUTPUT_PATH: &str = "checker_out.txt";
        const TARGET_CHECKER_ERROR_PATH: &str = "checker_err.txt";

        const TARGET_CHECKER_PATH: &str = "checker.out";
        const TARGET_SOLUTION_PATH: &str = "solution.out";

        Arc::clone(&self.sandbox)
            .write_group_into_box(
                vec![
                    (File::open(&*src_input_path).await?, TARGET_INPUT_PATH),
                    (File::open(&*src_checker_path).await?, TARGET_CHECKER_PATH),
                    (File::open(&*src_solution_path).await?, TARGET_SOLUTION_PATH),
                ]
                .into_iter()
                .map(|(from, to)| (from, Box::from(to)))
                .collect(),
            )
            .await?;

        let mut solution_cmd = self.lang.command_to_run(TARGET_SOLUTION_PATH);
        solution_cmd
            .time(Limited(self.limits.time))
            .memory(Limited(self.limits.memory))
            .real_time(Limited(self.limits.real_time));
        if let Some(stack) = self.limits.stack {
            solution_cmd.stack(Limited(stack));
        }
        solution_cmd
            .count_files(Limited(4))
            .count_process(Limited(1))
            .stdin(TARGET_INPUT_PATH)
            .stdout(TARGET_OUTPUT_PATH);

        let solution_result = match self.sandbox.run(&solution_cmd).await {
            Ok(res) => res,
            Err(e) => {
                log::error!("({log_state}) solution run error: {e:?}");
                return Err(e);
            }
        };

        let mut output_file = self.sandbox.read_from_box(TARGET_OUTPUT_PATH).await?;
        let mut output = String::new();
        output_file.read_to_string(&mut output).await?;
        let output = Arc::from(output.as_str());

        if let Some(verdict) = test::Verdict::from_run_status(solution_result.status) {
            return Ok(test::Result {
                verdict,
                time: solution_result.time,
                memory: solution_result.memory,
                output,
                message: Arc::from(
                    format!(
                        "ISOLATE: {}",
                        solution_result.status_message.unwrap_or(Box::from("-"))
                    )
                    .as_str(),
                ),
            });
        }

        if let Ok(mut correct) = File::open(&*src_correct_path).await {
            self.sandbox
                .write_into_box(&mut correct, TARGET_CORRECT_PATH)
                .await?;
        } else {
            log::debug!("({log_state}) correct file not founded");
        }

        let mut checker_cmd = Command::new(format!("./{TARGET_CHECKER_PATH}"));
        checker_cmd
            .args([TARGET_INPUT_PATH, TARGET_OUTPUT_PATH, TARGET_CORRECT_PATH])
            .count_files(Unlimited)
            .count_process(Unlimited)
            .stdout(TARGET_CHECKER_OUTPUT_PATH)
            .stderr(TARGET_CHECKER_ERROR_PATH);

        let checker_result = match self.sandbox.run(&checker_cmd).await {
            Ok(res) => res,
            Err(e) => {
                log::error!("({log_state}) checker error: {e:?}");
                return Err(e);
            }
        };

        let sandbox_clone = Arc::clone(&self.sandbox);
        let checker_output_handler: JoinHandle<Result<String>> = tokio::spawn(async move {
            let mut output = String::new();
            sandbox_clone
                .read_from_box(TARGET_CHECKER_OUTPUT_PATH)
                .await?
                .read_to_string(&mut output)
                .await?;
            Ok(output)
        });

        let sandbox_clone = Arc::clone(&self.sandbox);
        let checker_error_handler: JoinHandle<Result<String>> = tokio::spawn(async move {
            let mut output = String::new();
            sandbox_clone
                .read_from_box(TARGET_CHECKER_ERROR_PATH)
                .await?
                .read_to_string(&mut output)
                .await?;
            Ok(output)
        });

        let checker_output = checker_output_handler.await?.unwrap_or("-".to_string());
        let checker_error = checker_error_handler.await?.unwrap_or("-".to_string());

        let (verdict, message) = match checker_result.status {
            RunStatus::Ml | RunStatus::Tl | RunStatus::Sg(_) => (
                test::Verdict::Te,
                format!(
                    "checker_output: {checker_output}\n, checker_error: {checker_error}\n 'isolate': {}",
                    checker_result.status_message.as_deref().unwrap_or("")
                ),
            ),
            RunStatus::Ok => (
                test::Verdict::Ok,
                format!(
                    "checker_output: {checker_output}\n, checker_error: {checker_error}\n 'isolate': {}",
                    checker_result.status_message.as_deref().unwrap_or("")
                ),
            ),
            RunStatus::Re(code) => (
                match code {
                    1 => test::Verdict::Wa,
                    2 => test::Verdict::Pe,
                    _ => test::Verdict::Te,
                },
                format!("checker_output: {checker_output}\n, checker_error: {checker_error}"),
            ),
        };

        let result = test::Result {
            verdict,
            message: Arc::from(message),

            output,
            memory: solution_result.memory,
            time: solution_result.time,
        };

        log::info!("({log_state}) judgement result:\n{result:#?}");

        Ok(result)
    }
}
