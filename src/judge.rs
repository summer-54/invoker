const COMPILATION_TIME_LIMIT: f64 = 10.;

use serde::{Deserialize, Serialize};
use tokio::{
    fs::{File, create_dir, remove_dir_all},
    io::AsyncReadExt,
    sync::{Mutex, Semaphore, mpsc::UnboundedSender},
    task::JoinHandle,
};

use std::{fs::Permissions, os::unix::fs::PermissionsExt, sync::Arc};

use crate::{
    LogState, Result,
    sandboxes::isolate::{self, MaybeLimited, RunConfig, RunStatus, Sandbox},
};

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
enum ProblemType {
    Standart,
}

#[derive(Debug, Deserialize)]
struct ProblemLimits {
    time: f64,
    real_time: f64,

    memory: u64,
    stack: usize,
}

#[derive(Debug, Deserialize, Clone)]
struct TestsRange(usize, usize);

#[derive(Debug, Deserialize, Clone)]
struct Group {
    id: usize,
    range: TestsRange,
    cost: usize,
    depends: Box<[usize]>,
}

#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    #[serde(rename = "g++")]
    Gpp,
    #[serde(rename = "python3")]
    Python,
}

impl Lang {
    pub fn compile_command(&self, name: &str, result: &str) -> Box<str> {
        match self {
            Self::Gpp => format!("/usr/bin/g++ {name} -o {result} -Wall -O2 -lm"),
            Self::Python => format!("/usr/bin/cp --update=none {name} {result}"),
        }
        .into_boxed_str()
    }

    pub fn run_command(&self, name: &str) -> Box<str> {
        match self {
            Self::Gpp => format!("./{name}"),
            Self::Python => format!("/usr/bin/python3 {name}"),
        }
        .into_boxed_str()
    }
}

#[derive(Debug, Deserialize)]
struct ProblemConfig {
    r#type: ProblemType,
    lang: Lang,
    limits: ProblemLimits,
    groups: Box<[Group]>,
}

#[derive(Debug, Clone)]
pub enum FullResult {
    Ok {
        score: usize,
        groups_score: Box<[usize]>,
    },
    Ce(Box<str>),
    Te(Box<str>),
}

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
    pub fn match_error(status: isolate::RunStatus) -> Option<Self> {
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

pub struct Service {
    work_dir: Box<str>,

    semaphore: Semaphore,
    isolate: Arc<isolate::Service>,
    handler: Mutex<Option<JoinHandle<()>>>,
}

const INPUT_DIR: &str = "input";
const INPUT_EXT: Option<&str> = Some("txt");

const CORRECT_DIR: &str = "correct";
const CORRECT_EXT: Option<&str> = Some("txt");

const CHECKER_NAME: &str = "checker";
const CHECKER_EXT: Option<&str> = Some("out");

const SOLUTION_NAME: &str = "solution";
const SOLUTION_EXT: Option<&str> = Some("out");

fn path_from(dir: &str, name: &str, ext: Option<&str>) -> Box<str> {
    format!(
        "{dir}/{name}{}",
        ext.map(|s| [".", s].concat()).unwrap_or("".to_string())
    )
    .into_boxed_str()
}

impl Service {
    pub async fn new(isolate: Arc<isolate::Service>, work_dir: Box<str>) -> Service {
        if !tokio::fs::try_exists(&*work_dir).await.unwrap() {
            create_dir(&*work_dir).await.unwrap();
        }
        Service {
            work_dir,
            isolate,
            handler: Mutex::new(None),
            semaphore: Semaphore::new(1),
        }
    }

    async fn test(
        &self,
        sandbox: Sandbox,
        problem_config: Arc<ProblemConfig>,
        test_id: usize,
        lang: Lang,
    ) -> Result<TestResult> {
        let mut log_st = LogState::new();
        log_st = log_st.push("box", &*format!("{}", sandbox.id()));
        log_st = log_st.push("test", &*format!("{test_id}"));

        let limits = &problem_config.limits;
        let result = match problem_config.r#type {
            ProblemType::Standart => {
                let log_st = log_st.push("task type", "STANDART");
                log::trace!("({log_st}) testing STARTED");
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
                const TARGET_CHECKER_OUTPUT_PATH: &str = "cheker_out.txt";

                const TARGET_CHECKER_PATH: &str = "checker.out";
                const TARGET_SOLUTION_PATH: &str = "solution.out";

                sandbox
                    .write_into_box(&mut File::open(&*src_input_path).await?, TARGET_INPUT_PATH)
                    .await?;
                sandbox
                    .write_into_box(
                        &mut File::open(&*src_checker_path).await?,
                        TARGET_CHECKER_PATH,
                    )
                    .await?;
                sandbox
                    .write_into_box(
                        &mut File::open(&*src_solution_path).await?,
                        TARGET_SOLUTION_PATH,
                    )
                    .await?;

                let solution_result = match sandbox
                    .run(
                        lang.run_command(TARGET_SOLUTION_PATH),
                        RunConfig {
                            time_limit: MaybeLimited::Limited(limits.time),
                            memory_limit: MaybeLimited::Limited(limits.memory),
                            real_time_limit: limits.real_time,
                            extra_time_limit: None,
                            stack_limit: Some(MaybeLimited::Limited(limits.stack)),
                            open_files_limit: None,
                            process_limit: None,
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
                        log::error!("({log_st}) solution run erorr: {e:?}");
                        return Err(e);
                    }
                };

                let mut output_file = sandbox.read_from_box("out.txt").await?;
                let mut output = String::new();
                output_file.read_to_string(&mut output).await?;
                let output = Arc::from(output.as_str());

                if let Some(verdict) = Verdict::match_error(solution_result.status) {
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
                    log::debug!("({log_st}) correct file not founded");
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
                            stderr: None,
                        },
                    )
                    .await
                {
                    Ok(res) => res,
                    Err(e) => {
                        log::error!("({log_st}) checker erorr: {e:?}");
                        return Err(e);
                    }
                };

                let mut checker_output_file =
                    sandbox.read_from_box(TARGET_CHECKER_OUTPUT_PATH).await?;
                let mut checker_output = String::new();
                checker_output_file
                    .read_to_string(&mut checker_output)
                    .await?;
                let (verdict, message) = match checker_result.status {
                    RunStatus::Ml | RunStatus::Tl | RunStatus::Sg(_) => (
                        Verdict::Te,
                        format!(
                            "CHECKER: {checker_output} \n 'isolate': {}",
                            checker_result.status_message.as_deref().unwrap_or("")
                        ),
                    ),
                    RunStatus::Ok => (
                        Verdict::Ok,
                        format!(
                            "CHECKER: {checker_output} \n 'isolate': {}",
                            checker_result.status_message.as_deref().unwrap_or("")
                        ),
                    ),
                    RunStatus::Re(code) => (
                        match code {
                            1 => Verdict::Wa,
                            2 => Verdict::Pe,
                            _ => Verdict::Te,
                        },
                        format!("CHECKER: {checker_output}"),
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
        log::trace!("({log_st}) testing ENDED",);
        drop(sandbox);
        Ok(result)
    }

    pub async fn judge<R: Unpin + tokio::io::AsyncRead>(
        self: Arc<Self>,
        mut package: tokio_tar::Archive<R>,
        sender: UnboundedSender<(usize, TestResult)>,
    ) -> Result<FullResult> {
        let permit = self.semaphore.try_acquire()?;
        package.unpack(&*self.work_dir).await?;

        let mut text = String::new();
        File::open(&format!("{}/config.yaml", &self.work_dir))
            .await?
            .read_to_string(&mut text)
            .await?;
        let problem_config: Arc<ProblemConfig> = Arc::new(serde_yml::from_str(text.as_str())?);
        let lang = problem_config.lang;

        let sandbox = Arc::clone(&self.isolate).init_box().await?;

        let mut log_st = LogState::new();
        log_st = log_st.push("box", &*format!("{}", sandbox.id()));

        sandbox
            .write_into_box(
                &mut File::open(format!("{}/solution", &*self.work_dir)).await?,
                "solution.cpp",
            )
            .await?;

        let compile_errors_path = "compile_errors";
        let compile_command = lang.compile_command("solution.cpp", "solution.out");
        log::info!("compile command: {compile_command}");

        let compile_result = sandbox
            .run(
                compile_command,
                RunConfig {
                    open_files_limit: Some(MaybeLimited::Unlimited),
                    time_limit: MaybeLimited::Limited(COMPILATION_TIME_LIMIT),
                    process_limit: Some(MaybeLimited::Unlimited),
                    env: true,

                    stderr: Some(compile_errors_path.to_string().into_boxed_str()),
                    ..Default::default()
                },
            )
            .await?;

        log::info!("({log_st}) compiling");

        match compile_result.status {
            isolate::RunStatus::Tl | isolate::RunStatus::Ml | isolate::RunStatus::Sg(_) => {
                let mut message = String::new();
                if let Ok(mut r) = sandbox.read_from_box(compile_errors_path).await {
                    r.read_to_string(&mut message).await?;
                }
                return Ok(FullResult::Te(message.into_boxed_str()));
            }
            isolate::RunStatus::Re(_) => {
                let mut message = String::new();
                if let Ok(mut r) = sandbox.read_from_box(compile_errors_path).await {
                    r.read_to_string(&mut message).await?;
                }

                return Ok(FullResult::Ce(message.into_boxed_str()));
            }
            _ => (),
        };

        let mut file = tokio::fs::File::create(format!("{}/solution.out", self.work_dir)).await?;
        tokio::io::copy(&mut sandbox.read_from_box("solution.out").await?, &mut file).await?;
        file.set_permissions(Permissions::from_mode(0o777)).await?;

        drop(sandbox);

        let mut handlers: Vec<JoinHandle<Result<()>>> = vec![];
        // let blocked_tests =
        //     Arc::new(Mutex::new(vec![false; test_counts].into_boxed_slice()));
        let blocked_groups = Arc::new(Mutex::new(
            vec![None; problem_config.groups.len()].into_boxed_slice(),
        ));
        for group in problem_config.groups.clone() {
            'test: for test_number in (group.range.0 - 1)..group.range.1 {
                let mut log_st = LogState::new();
                log_st = log_st.push("test", &*format!("{test_number}"));
                log::trace!("({log_st}) looking on test");
                if blocked_groups.lock().await[group.id].is_some() {
                    continue;
                }
                for depend in &group.depends {
                    if blocked_groups.lock().await[*depend].is_some() {
                        continue 'test;
                    }
                }
                log::trace!("({log_st}) test started");

                let sandbox = Arc::clone(&self.isolate).init_box().await?;

                let blocked_groups = Arc::clone(&blocked_groups);
                let self_clone = Arc::clone(&self);
                let sender = sender.clone();

                let problem_config = Arc::clone(&problem_config);

                handlers.push(tokio::spawn(async move {
                    let result = self_clone
                        .test(sandbox, problem_config, test_number, lang)
                        .await?;
                    sender.send((test_number + 1, result.clone())).unwrap();
                    if !result.verdict.is_success() {
                        let block = &mut blocked_groups.lock().await[group.id];
                        if let Some(id) = block {
                            *block = Some(std::cmp::min(*id, test_number));
                        } else {
                            *block = Some(test_number);
                        }
                    }
                    Ok(())
                }));
            }
        }

        log::trace!("waiting all test processes");

        for handler in handlers {
            handler.await??;
        }
        let blocked_groups = blocked_groups.lock().await;

        let groups_score: Box<[usize]> = (0..problem_config.groups.len())
            .into_iter()
            .map(|i| {
                if blocked_groups[i].is_none() {
                    problem_config.groups[i].cost
                } else {
                    0
                }
            })
            .collect();

        let result = FullResult::Ok {
            score: groups_score.iter().sum(),
            groups_score,
        };

        log::info!("full result: {result:?}");

        remove_dir_all(&*self.work_dir).await?;
        tokio::fs::create_dir(&*self.work_dir).await?;
        drop(permit);
        Ok(result)
    }

    pub async fn stop_all(&self) -> Result<()> {
        self.semaphore.close();
        if let Some(handler) = &*self.handler.lock().await {
            handler.abort();
        }

        Arc::clone(&self.isolate).clean().await;
        Ok(())
    }
}

#[tokio::test]
async fn parsing() {
    let mut text = String::new();
    File::open(&format!("templates/problem_template/config.yaml"))
        .await
        .unwrap()
        .read_to_string(&mut text)
        .await
        .unwrap();
    let problem_config: ProblemConfig = serde_yml::from_str(text.as_str()).unwrap();
}
