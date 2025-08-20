use std::{
    collections::{HashMap, HashSet},
    io::{ErrorKind, Read},
    str::FromStr,
    sync::Arc,
};

use {
    futures::Stream,
    serde::de::DeserializeOwned,
    tokio::{
        io::AsyncRead,
        sync::{
            Mutex,
            mpsc::{Receiver, Sender},
        },
    },
};

use crate::{Error, Result, anyhow, pull::Pull};

use {
    serde::{Deserialize, Serialize},
    tokio::{
        fs::File,
        io::AsyncWriteExt,
        process::{Child, Command},
    },
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

use tokio_websockets::Message;

#[derive(Debug, Serialize, Deserialize)]
pub struct IsolateConfig {
    sandboxes_count: usize,

    proccess_default_limit: MaybeLimited<usize>,
    stack_default_limit: MaybeLimited<usize>,
    extra_time_default_limit: f64,
    open_files_default_limit: MaybeLimited<usize>,

    box_root: Box<str>,
    lock_root: Box<str>,

    cg_root: Box<str>,
    first_uid: usize,
    first_gid: usize,
    num_boxes: usize,

    restricted_init: bool,
}

const ISOLATE_CONFIG_PATH: &str = "/usr/local/etc/isolate";
const CONFIG_NAME: &str = "isolate.yaml";
impl Default for IsolateConfig {
    fn default() -> Self {
        Self {
            sandboxes_count: 1,
            box_root: "/.invoker/isolate".to_string().into_boxed_str(),
            lock_root: "/run/isolate/locks".to_string().into_boxed_str(),
            cg_root: "/run/isolate/cgroup".to_string().into_boxed_str(),
            first_uid: 60000,
            first_gid: 60000,
            num_boxes: 1000,
            restricted_init: false,

            proccess_default_limit: Limited(1),
            extra_time_default_limit: 0.,
            open_files_default_limit: Limited(2),
            stack_default_limit: Unlimited,
        }
    }
}

impl IsolateConfig {
    pub async fn init(configs_dir: &str) -> IsolateConfig {
        let path = format!("{configs_dir}/{CONFIG_NAME}").into_boxed_str();
        let this = if !tokio::fs::try_exists(&*path).await.unwrap() {
            let this = IsolateConfig::default();

            tokio::fs::write(&*path, serde_yml::to_string(&this).unwrap())
                .await
                .unwrap();

            log::warn!("invoker config not found by path: {}", path);
            log::info!("invoker config was automaticly created by path: {}", path);

            this
        } else {
            serde_yml::from_str(&tokio::fs::read_to_string(&*path).await.unwrap()).unwrap()
        };

        let mut isolate_config_file = File::create(ISOLATE_CONFIG_PATH).await.unwrap();
        isolate_config_file
            .write_all(
                format!(
                    "
                    box_root={}\n
                    lock_root={}\n
                    cg_root={}\n
                    first_uid={}\n
                    first_gid={}\n
                    num_boxes={}\n
                    resistance_init={}\n
                    ",
                    this.box_root,
                    this.lock_root,
                    this.cg_root,
                    this.first_uid,
                    this.first_gid,
                    this.num_boxes,
                    this.restricted_init
                )
                .as_bytes(),
            )
            .await
            .unwrap();
        this
    }
}

pub struct Service {
    config: IsolateConfig,
    path: Box<str>,
    boxes_pull: Pull<usize>,
}

impl Service {
    pub async fn new(config_dir: &str) -> Result<Arc<Service>> {
        let path = std::env::var("ISOLATE_PATH")
            .expect("enviroment variable 'ISOLATE_PATH' not found")
            .into_boxed_str();

        // todo!("checking existing isolate");

        if !Command::new(&*path)
            .arg("--version")
            .status()
            .await?
            .success()
        {
            log::error!("isolate doesn't exist by path '{path}'");
            return Err(anyhow!("isolate doesn't exist by path '{path}'"));
        }

        let config = IsolateConfig::init(config_dir).await;

        Ok(Arc::new(Service {
            boxes_pull: (0..config.sandboxes_count).collect(),
            config,
            path,
        }))
    }

    pub fn box_dir_path(&self, box_id: usize) -> Box<str> {
        format!("{}/{box_id}", self.config.lock_root).into_boxed_str()
    }

    pub async fn init_box(self: Arc<Self>) -> Result<Sandbox> {
        let box_id = self.boxes_pull.take().await;
        log::info!("isolate/box<id: {box_id}> starting...");
        let output = Command::new(&*self.path)
            .arg("--init")
            .arg(format!("--box-id={box_id}"))
            .output()
            .await
            .unwrap();
        if output.status.success() {
            log::info!("isolate/box<id: {box_id}> started successfully");
            Ok(Sandbox {
                service: self,
                id: box_id,
            })
        } else {
            Err(anyhow!(
                "while initing box <id: {box_id}>, exitcode: {:?}, stderr:\n{:?}",
                output.status.code(),
                String::from_utf8(output.stderr),
            ))
        }
    }

    pub async fn clean(self: Arc<Self>) {
        log::info!("isolate cleannig...");
        let output = Command::new(&*self.path)
            .arg("--cleanup")
            .status()
            .await
            .unwrap();
    }
}

pub struct RunConfig {
    pub time_limit: MaybeLimited<f64>,   // Time limit (in seconds)
    pub memory_limit: MaybeLimited<u64>, // Memory limit (in KiB)
    pub real_time_limit: f64,            // Real time limit (in seconds)
    pub extra_time_limit: Option<f64>,   // Extra time limit (in seconds)
    pub stack_limit: Option<MaybeLimited<usize>>, // Stack limit (in KiB)
    pub open_files_limit: Option<MaybeLimited<usize>>,
    pub proccess_limit: Option<MaybeLimited<usize>>,
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
            proccess_limit: Default::default(),
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
    Te,
}

impl RunStatus {
    fn is_success(&self) -> bool {
        *self == Self::Ok
    }
}

pub struct RunResult {
    pub status: RunStatus,
    pub time: f64,
    pub real_time: f64,
    pub status_message: Option<Box<str>>,
    pub memory: usize,
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
        tokio::spawn(async move {
            service.boxes_pull.put(id);
            log::info!("isolate/box <id: {id}> returned to boxes pull");
        });
    }
}

impl Sandbox {
    pub fn id(&self) -> usize {
        self.id
    }
    fn inner_dir(&self) -> Box<str> {
        format!("{}/{}/box", self.service.config.lock_root, self.id).into_boxed_str()
    }

    pub async fn run(
        &self,
        target_command: Box<str>,
        input_path: Option<&str>,
        output_path: Option<&str>,
        error_path: Option<&str>,
        cfg: RunConfig,
    ) -> Result<RunResult> {
        let meta_path = format!("{}/meta", self.inner_dir());

        let mut command = Command::new(&*self.service.path);
        command
            .arg("--run")
            .arg(format!("\"{target_command}\""))
            .arg(format!("--box-id={}", self.id))
            .arg(format!("--meta={meta_path}"));

        if let Some(input_path) = input_path {
            command.arg(format!("--stdin={}", input_path));
        }
        if let Some(output_path) = output_path {
            command.arg(format!("--stdout={}", output_path));
        }
        if let Some(error_path) = error_path {
            command.arg(format!("--stderr={}", error_path));
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
            .stack_limit
            .unwrap_or(self.service.config.open_files_default_limit)
        {
            command.arg(format!("--open_files_limit={}", open_files_limit));
        }
        if let Limited(proccess_limit) = cfg
            .proccess_limit
            .unwrap_or(self.service.config.proccess_default_limit)
        {
            command.arg(format!("--proccess_limit={}", proccess_limit));
        }

        log::info!(
            "'{command:?}' executing in isolate/sanbox <id: {}>",
            self.id
        );

        let status = command.status();

        let meta = tokio::fs::read_to_string(meta_path)
            .await?
            .split("\n")
            .map(|s| {
                s.split_once(':')
                    .map(|(k, v)| (Box::from(k.trim()), Box::from(v.trim())))
            })
            .collect::<Option<HashMap<Box<str>, Box<str>>>>()
            .ok_or(anyhow!("incorrect meta file (so strange)"))?;

        Ok(RunResult {
            status: if let Some(status) = meta.get("status") {
                match &**status {
                    "RE" => RunStatus::Re(meta["exitcode"].parse()?),
                    "SG" => RunStatus::Sg(meta["exitsig"].parse()?),
                    "TO" => RunStatus::Tl,
                    _ => return Err(anyhow!("incorrect WebSocker")),
                }
            } else {
                RunStatus::Ok
            },
            time: meta["time"].parse()?,
            real_time: meta["time"].parse()?,
            status_message: meta.get("message").cloned(),
            memory: meta["max-rss"].parse()?,
            killed: meta["killed"].parse()?,
        })
    }

    pub async fn write_into_box<R: AsyncRead + Unpin + ?Sized>(
        &self,
        from: &mut R,
        to: &str,
    ) -> Result<()> {
        _ = tokio::io::copy(
            from,
            &mut File::create(format!("{}/{to}", self.inner_dir())).await?,
        )
        .await?;
        log::info!("file '{to}' in box box_id was writed");
        Ok(())
    }

    pub async fn read_from_box(&self, from: &str) -> Result<File> {
        Ok(tokio::fs::File::open(format!("{}/{from}", self.inner_dir())).await?)
    }
}
