pub mod api;
// mod double_run;
mod interactive;
mod standard;

use crate::prelude::*;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tar_archive_rs as archive;
use tokio::{
    fs::{File, create_dir, create_dir_all, remove_dir_all},
    io::AsyncReadExt,
    sync::{Mutex, Semaphore, mpsc::UnboundedSender},
    task::JoinHandle,
};

use std::{collections::HashMap, fs::Permissions, os::unix::fs::PermissionsExt, sync::Arc};

use crate::{
    LogState, Result,
    sandbox::{self, Command, MaybeLimited},
};
use configo::Config as _;

use api::{
    submission::{self, Task},
    test,
};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, Hash, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    #[serde(rename = "g++")]
    Gpp,
    #[serde(rename = "python3")]
    Python,
}

impl Lang {
    pub fn command_to_run(&self, name: &str) -> Command {
        match self {
            Self::Gpp => Command::new(format!("./{name}")),
            Self::Python => {
                let mut cmd = Command::new("/usr/bin/python3");
                cmd.arg(name);
                cmd
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    compilation_commands: HashMap<Lang, Box<[Box<str>]>>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            compilation_commands: vec![
                (
                    Lang::Gpp,
                    vec![
                        "/usr/bin/g++",
                        "$SOURCE",
                        "-o",
                        "$OUTPUT",
                        "-O2",
                        "-Wall",
                        "-lm",
                    ]
                    .into(),
                ),
                (
                    Lang::Python,
                    vec!["/usr/bin/cp", "--update=none", "$SOURCE", "$OUTPUT"],
                ),
            ]
            .into_iter()
            .map(|(k, v)| (k, v.into_iter().map(|s| s.into()).collect()).into())
            .collect(),
        }
    }
}

impl configo::Config for Config {
    const NAME: &'static str = "judge";
}

impl Config {
    pub fn compilation_command(&self, lang: Lang, name: &str, result: &str) -> Result<Command> {
        let mut args = self
            .compilation_commands
            .get(&lang)
            .ok_or(anyhow!(
                "cannot find compilation command for lang: {lang:?} in judge config"
            ))?
            .iter()
            .map(|s| s.replace("$SOURCE", name).replace("$OUTPUT", result));
        let mut command = Command::new(args.next().ok_or(anyhow!(
            "cannot find program name for lang: {lang:?} in judge config"
        ))?);
        command.args(args);
        Ok(command)
    }
}

pub struct Service {
    config: Config,
    work_dir: Box<str>,

    semaphore: Semaphore,
    sandboxes: Arc<sandbox::Service>,
    handler: Mutex<Option<JoinHandle<()>>>,
}

const CHANNEL_DIR: &str = "/.invoker";
const SOLUTION_NAME: &str = "solution";
const SOLUTION_EXT: Option<&str> = Some("out");

pub fn path_from(dir: &str, name: &str, ext: Option<&str>) -> Box<str> {
    format!(
        "{dir}/{name}{}",
        ext.map(|s| [".", s].concat()).unwrap_or("".to_string())
    )
    .into_boxed_str()
}

#[async_trait]
pub trait Enviroment: Send {
    async fn run(self: Box<Self>) -> Result<test::Result>;
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
        if !tokio::fs::try_exists(CHANNEL_DIR).await.unwrap() {
            create_dir_all(CHANNEL_DIR).await.unwrap();
        }
        Service {
            config: Config::load(config_dir).await,
            work_dir,
            sandboxes,
            handler: Mutex::new(None),
            semaphore: Semaphore::new(1),
        }
    }

    pub async fn cancel_all_tests(&self) -> Result<()> {
        self.semaphore.close();
        if let Some(handler) = &*self.handler.lock().await {
            handler.abort();
        }

        Arc::clone(&self.sandboxes).clean().await;
        Ok(())
    }

    async fn compile_solution(&self, lang: Lang) -> Result<Option<submission::Result>> {
        let sandbox = Arc::clone(&self.sandboxes)
            .initialize_sandbox()
            .await
            .context("sandbox initializing")?;

        let mut log_state = LogState::new();
        log_state = log_state.push("box", &*format!("{}", sandbox.id()));

        sandbox
            .write_into_box(
                &mut File::open(format!("{}/solution", &*self.work_dir)).await?,
                "solution.cpp",
            )
            .await?;

        let compile_errors_path = "compile_errors";
        let mut compilation_command =
            self.config
                .compilation_command(lang, "solution.cpp", "solution.out")?;
        compilation_command
            .count_files(MaybeLimited::Unlimited)
            .count_process(MaybeLimited::Unlimited)
            .use_env()
            .stderr(compile_errors_path);

        let compile_result = sandbox.run(&compilation_command).await?;

        log::info!("({log_state}) compiling");

        match compile_result.status {
            sandbox::RunStatus::Tl | sandbox::RunStatus::Ml | sandbox::RunStatus::Sg(_) => {
                let mut message = String::new();
                if let Ok(mut r) = sandbox.read_from_box(compile_errors_path).await {
                    r.read_to_string(&mut message).await?;
                }
                return Ok(Some(submission::Result::Te(message.into_boxed_str())));
            }
            sandbox::RunStatus::Re(_) => {
                let mut message = String::new();
                if let Ok(mut r) = sandbox.read_from_box(compile_errors_path).await {
                    r.read_to_string(&mut message).await?;
                }

                return Ok(Some(submission::Result::Ce(message.into_boxed_str())));
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
        mut package: archive::Archive<R>,
        sender: UnboundedSender<(usize, test::Result)>,
    ) -> Result<submission::Result> {
        let permit = self.semaphore.try_acquire()?;
        log::info!("testing started");
        package.unpack(&*self.work_dir).await?;

        let mut text = String::new();
        File::open(&format!("{}/config.yaml", &self.work_dir))
            .await?
            .read_to_string(&mut text)
            .await?;

        log::trace!("config.yaml:\n{text}");

        let task: Arc<Task> = Arc::new(serde_yml::from_str(text.as_str())?);
        let lang = task.lang;

        if let Some(verdict) = self
            .compile_solution(lang)
            .await
            .context("solution compiling")?
        {
            return Ok(verdict);
        }

        let mut handlers: Vec<JoinHandle<Result<()>>> = vec![];

        let blocked_groups = Arc::new(Mutex::new(vec![None; task.groups.len()].into_boxed_slice()));

        for group in task.groups.clone() {
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

                let task = Arc::clone(&task);
                let enviroment = self
                    .prepare(task, test_number, log_state)
                    .await
                    .context("enviroment preparing")?;

                let blocked_groups = Arc::clone(&blocked_groups);
                let sender = sender.clone();

                handlers.push(tokio::spawn(async move {
                    let result = enviroment.run().await.context("enviroment running")?;
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

        let groups_score: Box<[usize]> = (0..task.groups.len())
            .into_iter()
            .map(|i| {
                if blocked_groups[i].is_none() {
                    task.groups[i].cost
                } else {
                    0
                }
            })
            .collect();

        let result = submission::Result::Ok {
            score: groups_score.iter().sum(),
            groups_score,
        };

        log::info!("full result: {result:?}");

        remove_dir_all(&*self.work_dir)
            .await
            .context("dirs cleaning")?;
        tokio::fs::create_dir(&*self.work_dir).await?;
        drop(permit);
        Ok(result)
    }

    async fn prepare(
        &self,
        task: Arc<Task>,
        test_id: usize,
        log_state: Arc<LogState>,
    ) -> Result<Box<dyn Enviroment>> {
        Ok(match task.r#type {
            submission::Type::Standard => Box::from(
                standard::prepare(
                    Arc::clone(&self.sandboxes),
                    task.lang,
                    task.limits,
                    self.work_dir.clone(),
                    test_id,
                    log_state,
                )
                .await
                .context("standart preparing")?,
            ) as Box<dyn Enviroment>,
            submission::Type::Interactive => Box::from(
                interactive::prepare(
                    Arc::clone(&self.sandboxes),
                    task.lang,
                    task.limits,
                    self.work_dir.clone(),
                    test_id,
                    log_state,
                )
                .await
                .context("interactive preparing")?,
            ) as Box<dyn Enviroment>,
        })
    }
}
