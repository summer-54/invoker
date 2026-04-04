mod application;
mod channel;
mod consts;
mod file;
mod judge;
mod logger;
#[cfg(feature = "mock")]
mod mock;
mod prelude;
mod sandbox;
mod server;

use prelude::*;

use logger::LogState;
use serde::Deserialize;
use uuid::Uuid;

use invoker_auth::{Cert, Parse};
use std::{path::Path, sync::Arc};

use crate::{
    application::App,
    server::stream::{
        AuthIncome, AuthOutgo, LoadIncome, LoadOutgo, MasterIncome, MasterOutgo, Stream,
    },
};

#[cfg(not(feature = "mock"))]
use crate::{
    server::websocket::{self, Uri},
    str::FromStr,
};

const VISIBLE_DATA_LEN: usize = 30;

fn short_slice_u8(data: &[u8]) -> &[u8] {
    &data[..std::cmp::min(data.len(), VISIBLE_DATA_LEN)]
}
#[derive(Clone, Deserialize, Debug)]
struct Config {
    #[cfg(feature = "mock")]
    pub loader_dir: Box<Path>,
    #[cfg(not(feature = "mock"))]
    pub manager_host: Box<str>,
    pub config_dir: Box<Path>,
    pub work_dir: Box<Path>,
    pub cache_dir: Box<Path>,

    pub isolate_exe_path: Box<Path>,
    pub cert_name: Box<str>,
    pub cert_path: Box<str>,
}

impl Config {
    pub async fn init() -> Result<Self> {
        let config = envy::prefixed("INVOKER_")
            .from_env::<Config>()
            .context("env config reading")?;
        log::debug!("environment variables:\n{config:#?}");
        Ok(config)
    }
}

struct Communication<
    A: Stream<AuthIncome, AuthOutgo>,
    M: Stream<MasterIncome, MasterOutgo>,
    L: Stream<LoadIncome, LoadOutgo>,
> {
    auth_stream: A,
    master_stream: M,
    load_stream: L,
}

#[cfg(not(feature = "mock"))]
async fn init_websocket_communication(
    config: Config,
) -> Result<(
    tokio::task::JoinHandle<Result<()>>,
    Communication<
        impl Stream<AuthIncome, AuthOutgo>,
        impl Stream<MasterIncome, MasterOutgo>,
        impl Stream<LoadIncome, LoadOutgo>,
    >,
)> {
    let channel = Arc::new(
        websocket::Channel::new(
            config.manager_host.as_ref(),
            Uri::from_str(format!("ws://{}", config.manager_host).as_str())?,
        )
        .await?,
    );

    let communication = Communication {
        auth_stream: channel.new_stream(consts::streams_names::AUTH).await,
        master_stream: channel.new_stream(consts::streams_names::MASTER).await,
        load_stream: channel.new_stream(consts::streams_names::LOAD).await,
    };

    let handler = tokio::spawn(channel.run());

    Ok((handler, communication))
}

#[cfg(feature = "mock")]
async fn init_mock_communication(
    config: Arc<Config>,
) -> (
    tokio::sync::mpsc::UnboundedSender<MasterIncome>,
    Communication<
        impl Stream<AuthIncome, AuthOutgo>,
        impl Stream<MasterIncome, MasterOutgo>,
        impl Stream<LoadIncome, LoadOutgo>,
    >,
) {
    use crate::mock::{AuthStream, LoadStream, MasterStream};

    log::info!("{} communication initialized", "mock".bold());
    let (sender, master_stream) = MasterStream::new();
    let load_stream = LoadStream::new(&config.loader_dir);
    (
        sender,
        Communication {
            auth_stream: AuthStream,
            master_stream: master_stream,
            load_stream: load_stream,
        },
    )
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    if !nix::unistd::Uid::current().is_root() {
        println!("{}", "must started as root".red().bold());
        bail!("must started as root");
    }

    let config = Arc::new(Config::init().await?);

    if !tokio::fs::try_exists(&*config.config_dir).await? {
        log::error!("config directory: '{:?}' not founded", config.config_dir);
        bail!("config directory: '{:?}' not founded", config.config_dir);
    }

    let judger_work_dir = config.work_dir.join("judge");
    let token = Uuid::new_v4();
    println!("\n[{}] invoker token\n", format!("{token}").yellow().bold());

    #[cfg(not(feature = "mock"))]
    let (_handler, communication) = init_websocket_communication(config.clone()).await?;

    #[cfg(feature = "mock")]
    let communication = {
        let mut args = std::env::args().skip(1);
        let (master_sender, communication) = init_mock_communication(Arc::clone(&config)).await;
        while let Some(name) = args.next() {
            let Some(lang) = args.next() else {
                bail!("lang was not founded")
            };
            let lang = judge::Lang::try_from(&*lang)?;

            let Some(package_id) = args.next() else {
                bail!("package id was not founded")
            };
            let package_id = package_id.parse::<file::Id>()?;

            let data = tokio::fs::read(&name)
                .await
                .context("reading file '{name}'")?
                .into_boxed_slice();
            master_sender.send(MasterIncome::Start {
                package_id,
                lang,
                data,
            })?;
        }
        communication
    };
    let cert = Cert::from_file(&*config.cert_path)?;
    let isolate_service =
        sandbox::Service::new(&config.config_dir, config.isolate_exe_path.clone()).await?;

    let inner_provider = file::stream_provider::StreamProvider::new(communication.load_stream);
    let file_provider =
        file::CachingProvider::init(config.cache_dir.clone(), inner_provider).await?;

    let app = App {
        master_stream: communication.master_stream,
        auth_stream: communication.auth_stream,
        judge_service: Arc::new(
            judge::Service::new(&config.config_dir, isolate_service, judger_work_dir).await,
        ),
        cert: Arc::new(cert),
        file_provider,
    };

    Stream::<MasterIncome, MasterOutgo>::send(
        &app.master_stream,
        MasterOutgo::Token {
            token,
            name: config.cert_name.clone(),
        },
    )
    .await?;

    let app = Arc::new(app);
    let result = app.run();
    // for name in std::env::args().skip(1) {
    //     app.start_judgment(
    //         tokio::fs::read(name.as_str())
    //             .await
    //             .context("reading file '{name}'")?
    //             .into_boxed_slice(),
    //     );
    // }

    let result = result.await;

    match result {
        Ok(_) => {
            Stream::<MasterIncome, MasterOutgo>::send(
                &app.master_stream,
                MasterOutgo::Exited {
                    code: 0,
                    data: Box::from(""),
                },
            )
            .await?
        }
        Err(e) => {
            log::error!("error: '{e:?}'");
            Stream::<MasterIncome, MasterOutgo>::send(
                &app.master_stream,
                MasterOutgo::Exited {
                    code: 1,
                    data: format!("{e:?}").into_boxed_str(),
                },
            )
            .await?
        }
    }
    Ok(())
}
