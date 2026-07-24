mod application;
mod channel;
mod judge;
#[cfg(feature = "mock")]
mod mock;
mod prelude;
mod serde_with;
mod server;
mod types;

use prelude::*;
use toaster_lib_rs::logger;

use logger::LogState;
use serde::Deserialize;
use uuid::Uuid;

use std::{path::Path, sync::Arc};
use toaster_lib_rs::auth::{Cert, Parse};

use crate::{
    application::App,
    server::stream::{
        AUTH_NAME, AuthIncome, AuthOutgo, JUDGE_NAME, JudgeIncome, JudgeOutgo, MASTER_NAME,
        MasterIncome, MasterOutgo, Stream,
    },
};

#[cfg(not(feature = "mock"))]
use {
    crate::server::websocket::{self, Uri},
    std::str::FromStr,
};
#[derive(Clone, Deserialize, Debug)]
struct Config {
    #[cfg(feature = "mock")]
    pub loader_dir: Box<Path>,
    #[cfg(not(feature = "mock"))]
    pub manager_host: Box<str>,
    pub config_dir: Box<Path>,
    pub work_dir: Box<Path>,

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
    J: Stream<JudgeIncome, JudgeOutgo>,
> {
    auth_stream: A,
    master_stream: M,
    judge_stream: J,
}

#[cfg(not(feature = "mock"))]
async fn init_websocket_communication(
    config: Arc<Config>,
) -> Result<(
    tokio::task::JoinHandle<Result<()>>,
    Communication<
        impl Stream<AuthIncome, AuthOutgo>,
        impl Stream<MasterIncome, MasterOutgo>,
        impl Stream<JudgeIncome, JudgeOutgo>,
    >,
)> {
    let channel = Arc::new(
        websocket::Channel::bind(
            config.manager_host.as_ref(),
            Uri::from_str(format!("ws://{}", config.manager_host).as_str())?,
        )
        .await?,
    );

    let communication = Communication {
        auth_stream: channel.new_stream(AUTH_NAME).await,
        master_stream: channel.new_stream(MASTER_NAME).await,
        judge_stream: channel.new_stream(JUDGE_NAME).await,
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
    use crate::mock::{AuthStream, MasterStream};

    log::info!("{} communication initialized", "mock".bold());
    let (sender, master_stream) = MasterStream::new();
    let load_stream = LoadStream::new(&config.loader_dir);
    (
        sender,
        Communication {
            auth_stream: AuthStream,
            master_stream,
            load_stream,
        },
    )
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    if !nix::unistd::Uid::current().is_root() {
        bail!("must started as root");
    }

    let config = Arc::new(Config::init().await?);

    if !tokio::fs::try_exists(&*config.config_dir).await? {
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
            let lang = judge::api::Lang::try_from(&*lang)?;

            let data = tokio::fs::read(&name)
                .await
                .context(format!("reading file '{name}'"))?
                .into_boxed_slice();
            master_sender.send(MasterIncome::Run { lang, data })?;
        }
        communication
    };
    let cert = Cert::from_file(&*config.cert_path)?;

    let app = App {
        master_stream: communication.master_stream,
        auth_stream: communication.auth_stream,
        judge_service: Arc::new(
            judge::Service::new(
                &config.config_dir,
                judger_work_dir,
                communication.judge_stream,
            )
            .await?,
        ),
        cert: Arc::new(cert),
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

    match app.run().await.context("App::run()") {
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
