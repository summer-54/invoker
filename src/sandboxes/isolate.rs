use std::{collections::HashMap, fs::Permissions, os::unix::fs::PermissionsExt, sync::Arc};

use crate::{
    LogState, Result, anyhow,
    config_loader::{self, Config as _},
};

use resource_pool::ResourcePool;

use serde::{Deserialize, Serialize};
use tokio::{
    fs::File,
    io::{AsyncRead, AsyncWriteExt},
    process::Command,
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
    stack_default_limit: MaybeLimited<usize>,
    extra_time_default_limit: f64,
    open_files_default_limit: MaybeLimited<usize>,

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
            extra_time_default_limit: 0.,
            open_files_default_limit: Limited(2),
            stack_default_limit: Unlimited,
        }
    }
}

impl config_loader::Config for IsolateConfig {
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
        if !Command::new(&*path)
            .arg("--version")
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
        log::info!("({log_state}) starting...");
        let output = Command::new(&*self.path)
            .arg("--init")
            .arg(format!("--box-id={box_id}"))
            .output()
            .await
            .unwrap();
        if output.status.success() {
            log::info!("({log_state}) started successfully");
            Ok(Sandbox {
                service: self,
                id: box_id,
            })
        } else {
            Err(anyhow!(
                "({log_state}) while initing, exitcode: {:?}, stderr:\n{:?}",
                output.status.code(),
                String::from_utf8(output.stderr),
            ))
        }
    }

    pub async fn clean(self: Arc<Self>) {
        log::info!("isolate cleannig...");
        let status = Command::new(&*self.path)
            .arg("--cleanup")
            .status()
            .await
            .unwrap();
        log::info!("isolate clean with status: {status}")
    }
}

pub struct RunConfig {
    pub time_limit: MaybeLimited<f64>,   // Time limit (in seconds)
    pub memory_limit: MaybeLimited<u64>, // Memory limit (in KiB)
    pub real_time_limit: f64,            // Real time limit (in seconds)
    pub extra_time_limit: Option<f64>,   // Extra time limit (in seconds)
    pub stack_limit: Option<MaybeLimited<usize>>, // Stack limit (in KiB)
    pub open_files_limit: Option<MaybeLimited<usize>>,
    pub process_limit: Option<MaybeLimited<usize>>,
    pub env: bool,

    pub stdin: Option<Box<str>>,
    pub stdout: Option<Box<str>>,
    pub stderr: Option<Box<str>>,
}

impl Default for RunConfig {
    fn default() -> Self {
        Self {
            time_limit: Default::default(),
            memory_limit: Default::default(),
            real_time_limit: 10.,
            extra_time_limit: Default::default(),
            stack_limit: Default::default(),
            open_files_limit: Default::default(),
            process_limit: Default::default(),
            env: false,

            stdin: Default::default(),
            stdout: Default::default(),
            stderr: Default::default(),
        }
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
            log::info!("({log_state}) returned to boxes pull");
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

    pub async fn run(&self, target_command: Box<str>, cfg: RunConfig) -> Result<RunResult> {
        let meta_path = format!("{}/meta", self.inner_dir());
        let mut log_st = LogState::new();
        log_st = log_st.push("box", &*format!("{}", self.id()));

        let mut command = Command::new(&*self.service.path);
        command
            .arg(format!("--box-id={}", self.id))
            .arg(format!("--meta={meta_path}"));

        if let Some(input_path) = cfg.stdin {
            command.arg(format!("--stdin={input_path}"));
        }
        if let Some(output_path) = cfg.stdout {
            let path = format!("{}/{}", self.inner_dir(), output_path);
            tokio::fs::File::create(path.clone()).await?;
            log::trace!("({log_st}) file: {path} created");
            command.arg(format!("--stdout={output_path}"));
        }
        if let Some(error_path) = cfg.stderr {
            let path = format!("{}/{}", self.inner_dir(), error_path);
            tokio::fs::File::create(path.clone()).await?;
            log::trace!("({log_st}) file: {path} created");
            command.arg(format!("--stderr={error_path}"));
        }

        if let Limited(time_limit) = cfg.time_limit {
            command.arg(format!("--time={}", time_limit));
        }
        if let Limited(memory_limit) = cfg.memory_limit {
            command.arg(format!("--mem={}", memory_limit));
        }
        command
            .arg(format!("--wall-time={}", cfg.real_time_limit))
            .arg(format!(
                "--extra-time={}",
                cfg.extra_time_limit
                    .unwrap_or(self.service.config.extra_time_default_limit)
            ));
        if let Limited(stack_limit) = cfg
            .stack_limit
            .unwrap_or(self.service.config.stack_default_limit)
        {
            command.arg(format!("--stack={}", stack_limit));
        }
        if let Limited(open_files_limit) = cfg
            .open_files_limit
            .unwrap_or(self.service.config.open_files_default_limit)
        {
            command.arg(format!("--open-files={}", open_files_limit));
        }
        if let Limited(process_limit) = cfg
            .process_limit
            .unwrap_or(self.service.config.process_default_limit)
        {
            command.arg(format!("--processes={}", process_limit));
        } else {
            command.arg(format!("--processes"));
        }

        if cfg.env {
            command.arg(format!("--full-env"));
        }

        command
            .arg("--run")
            .arg("--")
            .args(target_command.to_string().split_ascii_whitespace());

        log::info!("({log_st}) executing '{command:?}'");

        _ = command.status().await?;

        let meta = tokio::fs::read_to_string(meta_path).await?;
        log::trace!("({log_st}) meta file: {meta}");
        let meta = parse_meta_file(&meta);

        let result = RunResult {
            status: if let Some(status) = meta.get("status") {
                match &**status {
                    "RE" => RunStatus::Re(meta["exitcode"].parse()?),
                    "SG" => match meta["exitsig"].parse()? {
                        6 | 11 => RunStatus::Ml,
                        signal => RunStatus::Sg(signal),
                    },
                    "TO" => RunStatus::Tl,
                    _ => return Err(anyhow!("incorrect WebSocker")),
                }
            } else {
                RunStatus::Ok
            },
            time: meta["time"].parse()?,
            real_time: meta["time-wall"].parse()?,
            status_message: meta.get("message").cloned(),
            memory: meta["max-rss"].parse()?,
            killed: meta.get("killed").map(|s| &**s).unwrap_or("0") == "1",
        };

        log::info!("({log_st}) run result: {result:?}");

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
            let file = File::create(format!("{}/{to}", self.inner_dir())).await?;
            file.set_permissions(Permissions::from_mode(0o777)).await?;
            file
        })
        .await?;
        log::info!("({log_st}) '{to}' -> '{}/{to}'", self.inner_dir());
        Ok(())
    }

    pub async fn read_from_box(&self, from: &str) -> Result<File> {
        let mut log_st = LogState::new();
        log_st = log_st.push("box", &*format!("{}", self.id()));

        log::trace!("({log_st}) <- '{}/{from}'", self.inner_dir());
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
