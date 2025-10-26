use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::{fs::File, io::AsyncReadExt as _, task::JoinHandle};

use crate::{
    LogState, Result,
    judge::{Lang, ProblemConfig, ProblemType},
    sandboxes::isolate::{self, MaybeLimited, RunConfig, RunStatus, Sandbox},
};

const INPUT_DIR: &str = "input";
const INPUT_EXT: Option<&str> = Some("txt");

const CORRECT_DIR: &str = "correct";
const CORRECT_EXT: Option<&str> = Some("txt");

const CHECKER_NAME: &str = "checker";
const CHECKER_EXT: Option<&str> = Some("out");

const SOLUTION_NAME: &str = "solution";
const SOLUTION_EXT: Option<&str> = Some("out");

#[derive(Debug, Clone)]
pub struct TestResult {
    pub verdict: Verdict,
    pub time: f64,
    pub memory: u64,

    pub output: Arc<str>,
    pub message: Arc<str>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Ok, //ok
    Wa, //wrong answer
    Pe, //presentation error
    Ml, //memory limit
    Tl, //time limit
    Re, //runtime error
    Ce, //compile error
    Te, //testing system error
    Sl, //stack limit
}

impl Verdict {
    pub fn from_run_status(status: isolate::RunStatus) -> Option<Self> {
        Some(match status {
            isolate::RunStatus::Ok => return None,
            isolate::RunStatus::Tl => Self::Tl,
            isolate::RunStatus::Ml => Self::Ml,
            isolate::RunStatus::Re(_) => Self::Re,
            isolate::RunStatus::Sg(_) => Self::Re,
        })
    }

    pub fn is_success(&self) -> bool {
        *self == Verdict::Ok
    }
}

impl std::fmt::Display for Verdict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Verdict::Ok => "OK",
                Verdict::Wa => "WA",
                Verdict::Pe => "PE",
                Verdict::Ml => "ML",
                Verdict::Tl => "TL",
                Verdict::Re => "RE",
                Verdict::Ce => "CE",
                Verdict::Te => "TE",
                Verdict::Sl => "SL",
            }
        )
    }
}

fn path_from(dir: &str, name: &str, ext: Option<&str>) -> Box<str> {
    format!(
        "{dir}/{name}{}",
        ext.map(|s| [".", s].concat()).unwrap_or("".to_string())
    )
    .into_boxed_str()
}

impl super::Service {
    pub(super) async fn run(
        &self,
        sandbox: Sandbox,
        problem_config: Arc<ProblemConfig>,
        test_id: usize,
        lang: Lang,
    ) -> Result<TestResult> {
        let sandbox = Arc::new(sandbox);

        let mut log_state = LogState::new();
        log_state = log_state.push("box", &*format!("{}", sandbox.id()));
        log_state = log_state.push("test", &*format!("{test_id}"));

        let limits = &problem_config.limits;
        let result = match problem_config.r#type {
            ProblemType::Standard => {
                let log_state = log_state.push("task type", "STANDARD");
                log::trace!("({log_state}) testing STARTED");
                let src_input_path = path_from(
                    &format!("{}/{INPUT_DIR}", self.work_dir),
                    &format!("{}", test_id + 1),
                    INPUT_EXT,
                );
                let src_correct_path = path_from(
                    &format!("{}/{CORRECT_DIR}", self.work_dir),
                    &format!("{}", test_id + 1),
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
                {
                    let mut handlers = Vec::new();
                    let sandbox_clone = Arc::clone(&sandbox);
                    handlers.push(tokio::spawn(async move {
                        sandbox_clone
                            .write_into_box(
                                &mut File::open(&*src_input_path).await?,
                                TARGET_INPUT_PATH,
                            )
                            .await
                    }));

                    let sandbox_clone = Arc::clone(&sandbox);
                    handlers.push(tokio::spawn(async move {
                        sandbox_clone
                            .write_into_box(
                                &mut File::open(&*src_checker_path).await?,
                                TARGET_CHECKER_PATH,
                            )
                            .await
                    }));
                    let sandbox_clone = Arc::clone(&sandbox);
                    handlers.push(tokio::spawn(async move {
                        sandbox_clone
                            .write_into_box(
                                &mut File::open(&*src_solution_path).await?,
                                TARGET_SOLUTION_PATH,
                            )
                            .await
                    }));

                    for handler in handlers {
                        handler.await??;
                    }
                }
                let solution_result = match sandbox
                    .run(
                        lang.run_command(TARGET_SOLUTION_PATH),
                        RunConfig {
                            time_limit: MaybeLimited::Limited(limits.time),
                            memory_limit: MaybeLimited::Limited(limits.memory),
                            real_time_limit: limits.real_time,
                            extra_time_limit: None,
                            stack_limit: limits.stack.map(|s| MaybeLimited::Limited(s)),
                            open_files_limit: None,
                            process_limit: Some(MaybeLimited::Limited(1)),
                            env: false,

                            stdin: Some(TARGET_INPUT_PATH.to_string().into_boxed_str()),
                            stdout: Some(TARGET_OUTPUT_PATH.to_string().into_boxed_str()),
                            stderr: None,
                        },
                    )
                    .await
                {
                    Ok(res) => res,
                    Err(e) => {
                        log::error!("({log_state}) solution run error: {e:?}");
                        return Err(e);
                    }
                };

                let mut output_file = sandbox.read_from_box("out.txt").await?;
                let mut output = String::new();
                output_file.read_to_string(&mut output).await?;
                let output = Arc::from(output.as_str());

                if let Some(verdict) = Verdict::from_run_status(solution_result.status) {
                    return Ok(TestResult {
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
                    sandbox
                        .write_into_box(&mut correct, TARGET_CORRECT_PATH)
                        .await?;
                } else {
                    log::debug!("({log_state}) correct file not founded");
                }

                let checker_result = match sandbox
                        .run(
                            format!("./{TARGET_CHECKER_PATH} {TARGET_INPUT_PATH} {TARGET_OUTPUT_PATH} {TARGET_CORRECT_PATH}")
                                .into_boxed_str(),
                            RunConfig {
                                time_limit: MaybeLimited::Limited(limits.time),
                                memory_limit: MaybeLimited::Unlimited,
                                real_time_limit: limits.real_time,
                                extra_time_limit: None,
                                stack_limit: Some(MaybeLimited::Unlimited),
                                open_files_limit: Some(MaybeLimited::Unlimited),
                                process_limit: None,

                                env: false,

                                stdout: Some(TARGET_CHECKER_OUTPUT_PATH.to_string().into_boxed_str()),
                                stdin: None,
                                stderr: Some(TARGET_CHECKER_ERROR_PATH.to_string().into_boxed_str()),
                            },
                        )
                        .await
                    {
                        Ok(res) => res,
                        Err(e) => {
                            log::error!("({log_state}) checker error: {e:?}");
                            return Err(e);
                        }
                    };

                let sandbox_clone = Arc::clone(&sandbox);
                let checker_output_handler: JoinHandle<Result<String>> = tokio::spawn(async move {
                    let mut output = String::new();
                    sandbox_clone
                        .read_from_box(TARGET_CHECKER_OUTPUT_PATH)
                        .await?
                        .read_to_string(&mut output)
                        .await?;
                    Ok(output)
                });

                let sandbox_clone = Arc::clone(&sandbox);
                let checker_error_handler: JoinHandle<Result<String>> = tokio::spawn(async move {
                    let mut output = String::new();
                    sandbox_clone
                        .read_from_box(TARGET_CHECKER_ERROR_PATH)
                        .await?
                        .read_to_string(&mut output)
                        .await?;
                    Ok(output)
                });

                let checker_output = checker_output_handler.await??;
                let checker_error = checker_error_handler.await??;

                let (verdict, message) = match checker_result.status {
                    RunStatus::Ml | RunStatus::Tl | RunStatus::Sg(_) => (
                        Verdict::Te,
                        format!(
                            "checker_output: {checker_output}\n, checker_error: {checker_error}\n 'isolate': {}",
                            checker_result.status_message.as_deref().unwrap_or("")
                        ),
                    ),
                    RunStatus::Ok => (
                        Verdict::Ok,
                        format!(
                            "checker_output: {checker_output}\n, checker_error: {checker_error}\n 'isolate': {}",
                            checker_result.status_message.as_deref().unwrap_or("")
                        ),
                    ),
                    RunStatus::Re(code) => (
                        match code {
                            1 => Verdict::Wa,
                            2 => Verdict::Pe,
                            _ => Verdict::Te,
                        },
                        format!(
                            "checker_output: {checker_output}\n, checker_error: {checker_error}"
                        ),
                    ),
                };

                TestResult {
                    verdict,
                    message: Arc::from(message),

                    output,
                    memory: solution_result.memory,
                    time: solution_result.time,
                }
            }
        };
        log::trace!("({log_state}) testing ENDED",);
        drop(sandbox);
        Ok(result)
    }
}
