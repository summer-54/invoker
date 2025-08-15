use std::{
    collections::HashSet,
    io::{ErrorKind, Read},
    sync::Arc,
};

use anyhow::anyhow;
use futures::Stream;
use serde::de::DeserializeOwned;
use tokio::{
    io::AsyncRead,
    sync::{
        Mutex,
        mpsc::{Receiver, Sender},
    },
};

use crate::{Result, pull::Pull};

use {
    serde::{Deserialize, Serialize},
    tokio::{
        fs::File,
        io::AsyncWriteExt,
        process::{Child, Command},
    },
};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
enum MaybeLimited<T: Copy> {
    Limited(T),
    Unlimited,
}
use MaybeLimited::{Limited, Unlimited};

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
}

pub struct RunConfig {
    input_file: Box<str>,
    output_file: Box<str>,

    time_limit: MaybeLimited<f64>,   // Time limit (in seconds)
    memory_limit: MaybeLimited<u64>, // Memory limit (in KiB)
    real_time_limit: f64,            // Real time limit (in seconds)
    meta_file: Box<str>,             // Metadata file

    extra_time_limit: Option<f64>, // Extra time limit (in seconds)
    stack_limit: Option<MaybeLimited<usize>>, // Stack limit (in KiB)
    open_files_limit: Option<MaybeLimited<usize>>,
    proccess_limit: Option<MaybeLimited<usize>>,
}

pub struct RunResult {}

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
    fn inner_dir(&self) -> Box<str> {
        format!("{}/{}", self.service.config.lock_root, self.id).into_boxed_str()
    }

    pub async fn run(
        &self,
        target_command: Command,
        input_path: Option<&str>,
        output_path: Option<&str>,
        cfg: RunConfig,
    ) -> Result<RunResult> {
        todo!("copy input file into box");

        let mut command = Command::new(&*self.service.path);
        command.arg("--run").arg(format!("--box-id={}", self.id));
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

        let status = command.status();

        Ok(RunResult {})
    }

    pub async fn write_box<R: AsyncRead + Unpin + ?Sized>(
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

    pub async fn read_box<R: AsyncRead + Unpin + ?Sized>(&self, from: &str) -> Result<File> {
        Ok(tokio::fs::File::open(format!("{}/{from}", self.inner_dir())).await?)
    }
}
