const COMPILATION_TIME_LIMIT: f64 = 10.;

use tokio::io::AsyncReadExt;

use {
    serde::{Deserialize, Serialize},
    tokio::{
        fs::{File, remove_dir},
        sync::{Mutex, Semaphore, mpsc::UnboundedSender},
        task::JoinHandle,
    },
};

use std::{any::Any, sync::Arc};

use crate::{
    Result,
    sandboxes::isolate::{self, IsolateConfig, MaybeLimited, RunConfig, RunResult, Sandbox},
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ProblemType {
    Standart,
}

#[derive(Debug, Deserialize)]
struct ProblemLimits {
    time: f64,
    real_time: f64,

    memory: usize,
    stack: usize,
}

#[derive(Debug, Deserialize)]
struct TestsRange(usize, usize);

#[derive(Debug, Deserialize)]
struct Group {
    id: usize,
    range: TestsRange,
    cost: usize,
    depends: Box<[usize]>,
}

#[derive(Debug, Deserialize)]
struct TestsConfig {
    count: usize,
    groups: Box<[Group]>,
}

#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    #[serde(rename = "g++")]
    Gpp,
}

impl Lang {
    pub fn compile_command(&self, name: &str, result: &str) -> Box<str> {
        match self {
            Self::Gpp => format!("g++ {name} -o {result} -Wall -O2 -lm"),
        }
        .into_boxed_str()
    }
}

#[derive(Debug, Deserialize)]
struct ProblemConfig {
    r#type: ProblemType,
    lang: Lang,
    limits: ProblemLimits,
    tests: TestsConfig,
}

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
    pub memory: usize,

    pub output: Arc<str>,
    pub checker_output: Arc<str>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy)]
pub enum Verdict {
    Ok, //ok
    Wa, //wrong answer
    Ml, //memory limit
    Tl, //time limit
    Re, //runtime error
    Ce, //compile error
    Te, //testing system error
    Sl, //stack limit
}

impl std::fmt::Display for Verdict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Verdict::Ok => "OK",
                Verdict::Wa => "WA",
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

impl Service {
    pub fn new(isolate: Arc<isolate::Service>, work_dir: Box<str>) -> Service {
        Service {
            work_dir,
            isolate,
            handler: Mutex::new(None),
            semaphore: Semaphore::new(1),
        }
    }

    pub async fn test(&self, sandbox: Sandbox, lang: Lang) -> TestResult {
        todo!()
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
        let problem_config: ProblemConfig = serde_yml::from_str(text.as_str())?;
        let lang = problem_config.lang;
        match problem_config.r#type {
            ProblemType::Standart => {
                let sandbox = Arc::clone(&self.isolate).init_box().await?;
                sandbox
                    .write_into_box(
                        &mut File::open(format!("{}/solution.cpp", &*self.work_dir)).await?,
                        "solution.cpp",
                    )
                    .await?;

                let compile_errors_path = "compile_errors";
                let compile_result = sandbox
                    .run(
                        lang.compile_command("solution", "solution.out"),
                        None,
                        None,
                        Some(compile_errors_path),
                        RunConfig {
                            open_files_limit: Some(MaybeLimited::Unlimited),
                            ..Default::default()
                        },
                    )
                    .await?;

                log::info!("compiling isolate/sandbox<id: {}>", sandbox.id());

                match compile_result.status {
                    isolate::RunStatus::Tl | isolate::RunStatus::Ml | isolate::RunStatus::Sg(_) => {
                        let mut message = String::new();
                        sandbox
                            .read_from_box(compile_errors_path)
                            .await?
                            .read_to_string(&mut message);

                        return Ok(FullResult::Te(message.into_boxed_str()));
                    }
                    isolate::RunStatus::Re(_) => {
                        let mut message = String::new();
                        sandbox
                            .read_from_box(compile_errors_path)
                            .await?
                            .read_to_string(&mut message);

                        return Ok(FullResult::Ce(message.into_boxed_str()));
                    }
                    isolate::RunStatus::Te => {
                        return Ok(FullResult::Te(String::new().into_boxed_str()));
                    }
                    _ => (),
                }

                drop(sandbox);

                let test_counts = problem_config.tests.count;

                let blocks = vec![-1];

                let mut handles = vec![];
                let tests_blocked =
                    Arc::new(Mutex::new(vec![false; test_counts].into_boxed_slice()));
                let blocked_groups = Arc::new(Mutex::new(
                    vec![false; problem_config.tests.groups.len()].into_boxed_slice(),
                ));
                for group in problem_config.tests.groups {
                    for test_number in (group.range.0 - 1)..group.range.1 {
                        let sandbox = Arc::clone(&self.isolate).init_box().await?;
                        let self_clone = Arc::clone(&self);
                        let sender = sender.clone();
                        handles.push(tokio::spawn(async move {
                            let result = self_clone.test(sandbox, lang).await;
                            sender.send((test_number + 1, result.clone())).unwrap();
                            result
                        }));
                    }
                }
            }
        };

        remove_dir(&*self.work_dir).await?;
        drop(permit);
        todo!("run result")
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

    dbg!("{:#?}", problem_config);
}
