use {
    serde::{Deserialize, Serialize},
    tokio::{
        sync::{Mutex, Semaphore, mpsc::UnboundedSender},
        task::JoinHandle,
    },
};

use std::{io::Read, sync::Arc};

use crate::{App, Result, pull::Pull, sandboxes::isolate};

#[derive(Deserialize)]
enum ProblemType {
    Standart,
}

#[derive(Deserialize)]
struct ProblemLimits {
    time: f64,
    real_time: f64,

    memory: usize,
    stack: usize,
}

struct ProblemConfig {
    r#type: ProblemType,
    limits: ProblemLimits,
    // TODO: groups
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
    OK, //ok
    WA, //wrong answer
    ML, //memory limit
    TL, //time limit
    RE, //runtime error
    CE, //compile error
    TE, //testing system error
    SL, //stack limit
}

impl std::fmt::Display for Verdict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Verdict::OK => "OK",
                Verdict::WA => "WA",
                Verdict::ML => "ML",
                Verdict::TL => "TL",
                Verdict::RE => "RE",
                Verdict::CE => "CE",
                Verdict::TE => "TE",
                Verdict::SL => "SL",
            }
        )
    }
}

pub struct Service {
    semaphore: Semaphore,
    isolate: Arc<isolate::Service>,
    handler: Mutex<Option<JoinHandle<()>>>,
}

impl Service {
    pub fn new(isolate: Arc<isolate::Service>) -> Service {
        Service {
            isolate,
            handler: Mutex::new(None),
            semaphore: Semaphore::new(1),
        }
    }

    pub async fn judge<R: Unpin + tokio::io::AsyncRead>(
        &self,
        package: tokio_tar::Archive<R>,
        sender: UnboundedSender<TestResult>,
    ) -> Result<FullResult> {
        let permit = self.semaphore.try_acquire()?;

        tokio::spawn(async move { todo!("problem testing") });

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
