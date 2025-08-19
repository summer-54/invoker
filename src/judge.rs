use tokio::io::AsyncReadExt;

use {
    serde::{Deserialize, Serialize},
    tokio::{
        fs::{File, remove_dir},
        sync::{Mutex, Semaphore, mpsc::UnboundedSender},
        task::JoinHandle,
    },
};

use std::{io::Read, sync::Arc};

use crate::{Result, archive, sandboxes::isolate};

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

#[derive(Debug, Deserialize)]
struct ProblemConfig {
    r#type: ProblemType,
    limits: ProblemLimits,
    tests: TestsConfig,
}

pub struct FullResult {
    pub score: usize,
    pub groups_score: Box<[usize]>,
}

pub struct TestResult {
    pub id: usize,
    pub verdict: Verdict,
    pub time: f64,
    pub memory: usize,

    pub output: Arc<str>,
    pub checker_output: Arc<str>,
}

#[derive(Debug, Deserialize, Serialize)]
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

    pub async fn judge<R: Unpin + tokio::io::AsyncRead>(
        &self,
        mut package: tokio_tar::Archive<R>,
        sender: UnboundedSender<TestResult>,
    ) -> Result<FullResult> {
        let permit = self.semaphore.try_acquire()?;
        package.unpack(&*self.work_dir).await?;

        let mut text = String::new();
        File::open(&format!("{}/config.yaml", &self.work_dir))
            .await?
            .read_to_string(&mut text)
            .await?;
        let problem_config: ProblemConfig = serde_yml::from_str(text.as_str())?;

        match problem_config.r#type {
            ProblemType::Standart => {
                let sandbox = Arc::clone(&self.isolate).init_box().await?;

                // sandbox.run(target_command, input_path, output_path, cfg);

                let test_counts = problem_config.tests.count;

                for test_number in 1..=test_counts {}
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
