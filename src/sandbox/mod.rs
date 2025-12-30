pub mod command;
use anyhow::Context;
pub use command::Command;

use std::{
    collections::HashMap, fs::Permissions, os::unix::fs::PermissionsExt, process::Stdio, sync::Arc,
};

use crate::{LogState, Result, anyhow};
use colored::Colorize as _;
use configo::Config as _;

use resource_pool::ResourcePool;

use serde::{Deserialize, Serialize};
use tokio::{
    fs::File,
    io::{AsyncRead, AsyncWriteExt},
    process::Command as TokioCommand,
};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum MaybeLimited<T: Copy> {
    Limited(T),
    Unlimited,
}
use MaybeLimited::{Limited, Unlimited};

impl<T: Copy> Default for MaybeLimited<T> {
    fn default() -> Self {
        Unlimited
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IsolateConfig {
    sandboxes_count: usize,

    process_default_limit: MaybeLimited<usize>,
    open_files_default_limit: MaybeLimited<usize>,

    memory_default_limit: MaybeLimited<u64>,
    stack_default_limit: MaybeLimited<u64>,

    time_default_limit: MaybeLimited<f64>,
    extra_time_default_limit: f64,
    real_time_default_limit: MaybeLimited<f64>, // Real time limit (in seconds)

    box_root: Box<str>,
    lock_root: Box<str>,

    cg_root: Box<str>,
    first_uid: usize,
    first_gid: usize,

    restricted_init: bool,
}

const ISOLATE_CONFIG_PATH: &str = "/usr/local/etc/isolate";

impl Default for IsolateConfig {
    fn default() -> Self {
        Self {
            sandboxes_count: 1000,
            box_root: "/.invoker/isolate".to_string().into_boxed_str(),
            lock_root: "/run/isolate/locks".to_string().into_boxed_str(),
            cg_root: "/run/isolate/cgroup".to_string().into_boxed_str(),
            first_uid: 60000,
            first_gid: 60000,
            restricted_init: false,

            process_default_limit: Limited(1),
            open_files_default_limit: Limited(2),

            time_default_limit: Limited(10.),
            extra_time_default_limit: 0.,
            real_time_default_limit: Limited(10.),

            memory_default_limit: Limited(1 << 20),
            stack_default_limit: Unlimited,
        }
    }
}

impl configo::Config for IsolateConfig {
    const NAME: &'static str = "isolate";
}

impl IsolateConfig {
    pub async fn write_config_file(&self) {
        let mut isolate_config_file = File::create(ISOLATE_CONFIG_PATH).await.unwrap();
        isolate_config_file
            .write_all(
                format!(
                    "box_root={}\nlock_root={}\ncg_root={}\nfirst_uid={}\nfirst_gid={}\nnum_boxes={}\nrestricted_init={}\n",
                    self.box_root,
                    self.lock_root,
                    self.cg_root,
                    self.first_uid,
                    self.first_gid,
                    self.sandboxes_count,
                    if self.restricted_init {
                        1
                    } else {
                        0
                    }
                )
                .as_bytes(),
            )
            .await
            .unwrap();
    }
}

pub struct Service {
    config: IsolateConfig,
    path: Box<str>,
    boxes_pull: ResourcePool<usize>,
}

impl Service {
    pub async fn new(config_dir: &str, path: Box<str>) -> Result<Arc<Service>> {
        if !TokioCommand::new(&*path)
            .arg("--version")
            .stdout(Stdio::null())
            .status()
            .await?
            .success()
        {
            log::error!("isolate doesn't exist by path '{path}'");
            return Err(anyhow!("isolate doesn't exist by path '{path}'"));
        }

        let config = IsolateConfig::load(config_dir).await;
        config.write_config_file().await;

        Ok(Arc::new(Service {
            boxes_pull: (0..config.sandboxes_count).collect(),
            config,
            path,
        }))
    }

    pub async fn initialize_sandbox(self: Arc<Self>) -> Result<Sandbox> {
        let box_id = self.boxes_pull.take().await;
        let mut log_state = LogState::new();
        log_state = log_state.push("box", &*format!("{box_id}"));
        log::debug!("({log_state}) starting");
        let output = TokioCommand::new(&*self.path)
            .arg("--init")
            .arg(format!("--box-id={box_id}"))
            .output()
            .await
            .unwrap();
        if output.status.success() {
            log::debug!("({log_state}) started successfully");
            Ok(Sandbox {
                service: self,
                id: box_id,
            })
        } else {
            let err = String::from_utf8(output.stderr);
            log::error!(
                "box_id: {box_id} while initing, exitcode: {:?}, stderr:\n{:?}",
                output.status.code(),
                &err,
            );
            Err(anyhow!(
                "box_id: {box_id} while initing, exitcode: {:?}, stderr:\n{:?}",
                output.status.code(),
                err,
            ))
        }
    }

    pub async fn clean(self: Arc<Self>) {
        log::info!("isolate cleannig started");
        let status = TokioCommand::new(&*self.path)
            .arg("--cleanup")
            .status()
            .await
            .unwrap();
        log::info!("isolate cleaned with status: {status}")
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum RunStatus {
    Ok,
    Tl,
    Ml,
    Re(u8),
    Sg(u8),
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct RunResult {
    pub status: RunStatus,
    pub time: f64,
    pub real_time: f64,
    pub status_message: Option<Box<str>>,
    pub memory: u64,
    pub killed: bool,
}

pub struct Sandbox {
    service: Arc<Service>,
    id: usize,
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let service = Arc::clone(&self.service);
        let id = self.id;

        let log_state = LogState::new().push("box", &*format!("{id}"));
        tokio::spawn(async move {
            service.boxes_pull.put(id);
            log::trace!("({log_state}) returned to boxes pull");
        });
    }
}

fn parse_meta_file(s: &str) -> HashMap<Box<str>, Box<str>> {
    s.split("\n")
        .filter_map(|s| {
            s.split_once(':')
                .map(|(k, v)| (Box::from(k.trim()), Box::from(v.trim())))
        })
        .collect::<HashMap<Box<str>, Box<str>>>()
}

impl Sandbox {
    pub fn id(&self) -> usize {
        self.id
    }
    fn inner_dir(&self) -> Box<str> {
        format!("{}/{}/box", self.service.config.box_root, self.id).into_boxed_str()
    }

    pub async fn run(&self, target: &Command) -> Result<RunResult> {
        let target = target.clone();
        let meta_path = format!("{}/meta", self.inner_dir());
        let mut log_st = LogState::new();
        log_st = log_st.push("box", &*format!("{}", self.id()));

        let mut command = TokioCommand::new(&*self.service.path);
        command
            .arg(format!("--box-id={}", self.id))
            .arg(format!("--meta={meta_path}"))
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        if let Some(input_path) = target.stdin {
            command.arg(format!("--stdin={input_path}"));
        }
        if let Some(output_path) = target.stdout {
            if output_path.chars().nth(0).unwrap() != '/' {
                let path = format!("{}/{}", self.inner_dir(), output_path);
                tokio::fs::File::create(&path)
                    .await
                    .context("creating file")?;
            }
            log::trace!("({log_st}) file: {output_path} created");
            command.arg(format!("--stdout={output_path}"));
        }
        if let Some(error_path) = target.stderr {
            if error_path.chars().nth(0).unwrap() != '/' {
                let path = format!("{}/{}", self.inner_dir(), error_path);
                tokio::fs::File::create(&path)
                    .await
                    .context("creating file")?;
            }

            log::trace!("({log_st}) file: {error_path} created");
            command.arg(format!("--stderr={error_path}"));
        }

        for dir in target.open_dirs {
            command.arg(format!("--dir={dir}"));
        }

        if let Limited(time_limit) = target
            .time_limit
            .unwrap_or(self.service.config.time_default_limit)
        {
            command.arg(format!("--time={}", time_limit));
        }

        if let Limited(real_time_limit) = target
            .real_time_limit
            .unwrap_or(self.service.config.real_time_default_limit)
        {
            command.arg(format!("--wall-time={}", real_time_limit));
        }

        if let Limited(memory_limit) = target
            .memory_limit
            .unwrap_or(self.service.config.memory_default_limit)
        {
            command.arg(format!("--mem={}", memory_limit));
        }
        command.arg(format!(
            "--extra-time={}",
            target
                .extra_time_limit
                .unwrap_or(self.service.config.extra_time_default_limit)
        ));
        if let Limited(stack_limit) = target
            .stack_limit
            .unwrap_or(self.service.config.stack_default_limit)
        {
            command.arg(format!("--stack={}", stack_limit));
        }
        if let Limited(open_files_limit) = target
            .count_files_limit
            .unwrap_or(self.service.config.open_files_default_limit)
        {
            command.arg(format!("--open-files={}", open_files_limit));
        }
        if let Limited(process_limit) = target
            .count_process_limit
            .unwrap_or(self.service.config.process_default_limit)
        {
            command.arg(format!("--processes={}", process_limit));
        } else {
            command.arg(format!("--processes"));
        }

        if target.use_env {
            command.arg(format!("--full-env"));
        }

        command
            .arg("--run")
            .arg("--")
            .arg(target.program.to_string())
            .args(target.args.into_iter().map(|b| b.to_string()));

        log::trace!("({log_st}) executing:\n{command:#?}");

        _ = command.status().await.context("running command")?;

        let meta = tokio::fs::read_to_string(meta_path)
            .await
            .context("reading file '{meta_path}'")?;
        log::trace!("({log_st}) meta file:\n{meta}");
        let meta = parse_meta_file(&meta);

        let result = RunResult {
            status: if let Some(status) = meta.get("status") {
                match &**status {
                    "RE" => RunStatus::Re(meta["exitcode"].parse().context("parsing exitcode")?),
                    "SG" => match meta["exitsig"].parse().context("parsing exitsig")? {
                        6 | 11 => RunStatus::Ml,
                        signal => RunStatus::Sg(signal),
                    },
                    "TO" => RunStatus::Tl,
                    _ => return Err(anyhow!("incorrect meta file in ISOLATE")),
                }
            } else {
                RunStatus::Ok
            },
            time: meta["time"].parse().context("parsing time")?,
            real_time: meta["time-wall"].parse().context("parsing time-wall")?,
            status_message: meta.get("message").cloned(),
            memory: meta["max-rss"].parse().context("max-rss")?,
            killed: meta.get("killed").map(|s| &**s).unwrap_or("0") == "1",
        };

        log::trace!("({log_st}) run result:\n{result:#?}");

        Ok(result)
    }

    pub async fn write_into_box<R: AsyncRead + Unpin + ?Sized>(
        &self,
        from: &mut R,
        to: &str,
    ) -> Result<()> {
        let mut log_st = LogState::new();
        log_st = log_st.push("box", &*format!("{}", self.id()));

        _ = tokio::io::copy(from, &mut {
            let file = File::create(format!("{}/{to}", self.inner_dir()))
                .await
                .context("creating file")?;
            file.set_permissions(Permissions::from_mode(0o777))
                .await
                .context("setting permissions")?;
            file
        })
        .await?;
        log::trace!(
            "({log_st}) copied '{}' to '{}'",
            to.bold(),
            format!("{}/{to}", self.inner_dir()).bold()
        );
        Ok(())
    }

    pub async fn write_group_into_box<R: AsyncRead + Unpin + Send + 'static>(
        self: Arc<Self>,
        group: Box<[(R, Box<str>)]>,
    ) -> Result<()> {
        let mut handlers = Vec::new();
        for (mut from, to) in group {
            let this = Arc::clone(&self);
            handlers.push(tokio::spawn(async move {
                this.write_into_box(&mut from, &*to).await
            }));
        }

        for handler in handlers {
            handler.await??;
        }
        Ok(())
    }

    pub async fn read_from_box(&self, from: &str) -> Result<File> {
        let mut log_st = LogState::new();
        log_st = log_st.push("box", &*format!("{}", self.id()));

        log::trace!(
            "({log_st}) open '{}'",
            format!("{}/{from}", self.inner_dir()).bold()
        );
        Ok(tokio::fs::File::open(format!("{}/{from}", self.inner_dir())).await?)
    }
}

#[tokio::test]
pub async fn default_isolate_config() {
    panic!(
        "{}",
        serde_yml::to_string(&IsolateConfig::default()).unwrap()
    );
}

#[tokio::test]
async fn meta_file_parsing() {
    let meta = "time:0.185
time-wall:0.331
max-rss:254360
csw-voluntary:6
csw-forced:5
exitsig:11
status:SG
message:Caught fatal signal 11
"
    .split("\n")
    .filter_map(|s| {
        s.split_once(':')
            .map(|(k, v)| (Box::from(k.trim()), Box::from(v.trim())))
    })
    .collect::<Vec<(Box<str>, Box<str>)>>();
    // .ok_or(anyhow!("incorrect meta file (so strange)"))
    // .unwrap();

    panic!("{meta:?}");
}
