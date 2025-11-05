mod interactive;
mod standard;

use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use tokio::{
    fs::{File, create_dir, create_dir_all, remove_dir_all},
    io::AsyncReadExt,
    sync::{Mutex, Semaphore, mpsc::UnboundedSender},
    task::JoinHandle,
};

use std::{collections::HashMap, fs::Permissions, os::unix::fs::PermissionsExt, sync::Arc};

use crate::{
    LogState, Result,
    config_loader::{self, Config as _},
    sandbox::{self, MaybeLimited, RunConfig},
};

const COMPILATION_TIME_LIMIT: f64 = 10.;

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
enum ProblemType {
    Standard,
    Interactive,
}

#[derive(Debug, Deserialize, Clone, Copy)]
struct ProblemLimits {
    time: f64,
    real_time: f64,

    memory: u64,
    stack: Option<usize>,
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

#[derive(Debug, Serialize, Deserialize, Clone, Copy, Hash, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    #[serde(rename = "g++")]
    Gpp,
    #[serde(rename = "python3")]
    Python,
}

impl Lang {
    pub fn run_command(&self, name: &str) -> Box<str> {
        match self {
            Self::Gpp => format!("./{name}"),
            Self::Python => format!("/usr/bin/python3 {name}"),
        }
        .into_boxed_str()
    }
}

#[derive(Debug, Deserialize)]
struct Problem {
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

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    compilation_commands: HashMap<Lang, Box<str>>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            compilation_commands: vec![
                (Lang::Gpp, "/usr/bin/g++ $SOURCE -o $OUTPUT -O2 -Wall -lm"),
                (Lang::Python, "/usr/bin/cp --update=none $SOURCE $OUTPUT"),
            ]
            .into_iter()
            .map(|(k, v)| (k, Box::from(v)))
            .collect(),
        }
    }
}

impl config_loader::Config for Config {
    const NAME: &'static str = "judge";
}

impl Config {
    pub fn compilation_command(&self, lang: Lang, name: &str, result: &str) -> Result<Box<str>> {
        let command = self
            .compilation_commands
            .get(&lang)
            .ok_or(anyhow!(
                "cannot find compilation command for lang: {lang:?} in judge config"
            ))?
            .clone();

        Ok(command
            .replace("$SOURCE", name)
            .replace("$OUTPUT", result)
            .into_boxed_str())
    }
}

pub struct Service {
    config: Config,
    work_dir: Box<str>,

    semaphore: Semaphore,
    sandboxes: Arc<sandbox::Service>,
    handler: Mutex<Option<JoinHandle<()>>>,
}

impl Service {
    pub async fn new(
        config_dir: &str,
        sandboxes: Arc<sandbox::Service>,
        work_dir: Box<str>,
    ) -> Service {
        if !tokio::fs::try_exists(&*work_dir).await.unwrap() {
            create_dir(&*work_dir).await.unwrap();
        }
        if !tokio::fs::try_exists(interactive::CHANNEL_DIR)
            .await
            .unwrap()
        {
            create_dir_all(interactive::CHANNEL_DIR).await.unwrap();
        }
        Service {
            config: Config::load(config_dir).await,
            work_dir,
            sandboxes,
            handler: Mutex::new(None),
            semaphore: Semaphore::new(1),
        }
    }

    async fn compile_solution(&self, lang: Lang) -> Result<Option<FullResult>> {
        let sandbox = Arc::clone(&self.sandboxes).initialize_sandbox().await?;

        let mut log_state = LogState::new();
        log_state = log_state.push("box", &*format!("{}", sandbox.id()));

        sandbox
            .write_into_box(
                &mut File::open(format!("{}/solution", &*self.work_dir)).await?,
                "solution.cpp",
            )
            .await?;

        let compile_errors_path = "compile_errors";
        let compilation_command =
            self.config
                .compilation_command(lang, "solution.cpp", "solution.out")?;
        log::info!("compile command: {compilation_command}");

        let compile_result = sandbox
            .run(
                compilation_command,
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

        log::info!("({log_state}) compiling");

        match compile_result.status {
            sandbox::RunStatus::Tl | sandbox::RunStatus::Ml | sandbox::RunStatus::Sg(_) => {
                let mut message = String::new();
                if let Ok(mut r) = sandbox.read_from_box(compile_errors_path).await {
                    r.read_to_string(&mut message).await?;
                }
                return Ok(Some(FullResult::Te(message.into_boxed_str())));
            }
            sandbox::RunStatus::Re(_) => {
                let mut message = String::new();
                if let Ok(mut r) = sandbox.read_from_box(compile_errors_path).await {
                    r.read_to_string(&mut message).await?;
                }

                return Ok(Some(FullResult::Ce(message.into_boxed_str())));
            }
            _ => (),
        };

        let mut file = tokio::fs::File::create(format!("{}/solution.out", self.work_dir)).await?;
        tokio::io::copy(&mut sandbox.read_from_box("solution.out").await?, &mut file).await?;
        file.set_permissions(Permissions::from_mode(0o777)).await?;
        Ok(None)
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

        log::trace!("config.yaml: {text}");

        let problem: Arc<Problem> = Arc::new(serde_yml::from_str(text.as_str())?);
        let lang = problem.lang;

        if let Some(verdict) = self.compile_solution(lang).await? {
            return Ok(verdict);
        }

        let mut handlers: Vec<JoinHandle<Result<()>>> = vec![];

        let blocked_groups = Arc::new(Mutex::new(
            vec![None; problem.groups.len()].into_boxed_slice(),
        ));

        for group in problem.groups.clone() {
            'test: for test_number in (group.range.0 - 1)..group.range.1 {
                let mut log_state = LogState::new();
                log_state = log_state.push("test", &*format!("{test_number}"));
                log::trace!("({log_state}) looking on test");

                if blocked_groups.lock().await[group.id].is_some() {
                    continue;
                }
                for depend in &group.depends {
                    if blocked_groups.lock().await[*depend].is_some() {
                        continue 'test;
                    }
                }

                log::trace!("({log_state}) test started");

                let problem = Arc::clone(&problem);
                let enviroment = self.prepare(problem, test_number, log_state).await?;

                let blocked_groups = Arc::clone(&blocked_groups);
                let sender = sender.clone();

                handlers.push(tokio::spawn(async move {
                    let result = enviroment.run().await?;
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

        let groups_score: Box<[usize]> = (0..problem.groups.len())
            .into_iter()
            .map(|i| {
                if blocked_groups[i].is_none() {
                    problem.groups[i].cost
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

    pub async fn cancel_all_tests(&self) -> Result<()> {
        self.semaphore.close();
        if let Some(handler) = &*self.handler.lock().await {
            handler.abort();
        }

        Arc::clone(&self.sandboxes).clean().await;
        Ok(())
    }
}

use channel::Channel;
use std::pin::Pin;
use std::process::Output;

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
    pub fn from_run_status(status: sandbox::RunStatus) -> Option<Self> {
        Some(match status {
            sandbox::RunStatus::Ok => return None,
            sandbox::RunStatus::Tl => Self::Tl,
            sandbox::RunStatus::Ml => Self::Ml,
            sandbox::RunStatus::Re(_) => Self::Re,
            sandbox::RunStatus::Sg(_) => Self::Re,
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

pub fn path_from(dir: &str, name: &str, ext: Option<&str>) -> Box<str> {
    format!(
        "{dir}/{name}{}",
        ext.map(|s| [".", s].concat()).unwrap_or("".to_string())
    )
    .into_boxed_str()
}

pub trait Enviroment: Send {
    fn run<'a>(self: Box<Self>) -> Pin<Box<dyn Future<Output = Result<TestResult>> + Send + 'a>>;
}

impl Service {
    pub(super) async fn prepare(
        &self,
        problem: Arc<Problem>,
        test_id: usize,
        log_state: Arc<LogState>,
    ) -> Result<Box<dyn Enviroment>> {
        Ok(match problem.r#type {
            ProblemType::Standard => Box::from(
                standard::prepare(
                    Arc::clone(&self.sandboxes),
                    problem.lang,
                    problem.limits,
                    self.work_dir.clone(),
                    test_id,
                    log_state,
                )
                .await?,
            ),
            ProblemType::Interactive => Box::from(
                interactive::prepare(
                    Arc::clone(&self.sandboxes),
                    problem.lang,
                    problem.limits,
                    self.work_dir.clone(),
                    test_id,
                    log_state,
                )
                .await?,
            ),
        })
    }
}
