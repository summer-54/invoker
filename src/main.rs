mod application;
mod archive;
mod communication;
mod config_loader;
mod judge;
mod logger;
mod sandboxes;

use crate::{
    application::App,
    communication::{
        income,
        outgo::{self, Sender},
        websocket::Uri,
    },
};

pub use {
    anyhow::{Error, Result, anyhow},
    env_logger,
    logger::LogState,
    serde::{Deserialize, Serialize},
};

use {
    std::{str::FromStr, sync::Arc},
    uuid::Uuid,
};

#[derive(Clone, Deserialize, Debug)]
struct Config {
    pub manager_host: Box<str>,
    pub config_dir: Box<str>,
    pub work_dir: Box<str>,

    pub isolate_exe_path: Box<str>,
}

impl Config {
    pub async fn init() -> Result<Self> {
        let config = envy::prefixed("INVOKER_").from_env::<Config>()?;
        log::info!("environment variables: {config:#?}");
        Ok(config)
    }
}

#[cfg(not(feature = "mock"))]
async fn init_communnication(
    token: Uuid,
    config: Config,
) -> Result<(Arc<impl income::Receiver>, Arc<impl outgo::Sender>)> {
    let websocket_service = Arc::new(
        communication::websocket::Service::new(
            config.manager_host.as_ref(),
            Uri::from_str(format!("ws://{}", config.manager_host).as_str())?,
        )
        .await?,
    );

    websocket_service
        .send(communication::outgo::Msg::Token(token))
        .await?;

    Ok((websocket_service.clone(), websocket_service))
}

#[cfg(feature = "mock")]
async fn init_communnication(
    token: Uuid,
    config: Config,
) -> Result<(Arc<impl income::Receiver>, Arc<impl outgo::Sender>)> {
    log::info!("mock communication initialized");
    Ok((Arc::new(income::MockReceiver), Arc::new(outgo::MockSender)))
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let config = Config::init().await?;

    let judger_work_dir = format!("{}/judge", config.work_dir).into_boxed_str();
    let token = Uuid::new_v4();
    println!("invoker token: {token}");

    let (r, s) = init_communnication(token, config.clone()).await?;

    let isolate_service =
        sandboxes::isolate::Service::new(&config.config_dir, config.isolate_exe_path).await?;

    let app = App {
        receiver: r,
        sender: s,
        judge_service: Arc::new(
            judge::Service::new(&config.config_dir, isolate_service, judger_work_dir).await,
        ),
    };

    let app = Arc::new(app);
    let result = Arc::clone(&app).run();
    for name in std::env::args().skip(1) {
        app.start_judgment(tokio::fs::read(name.as_str()).await?.into_boxed_slice());
    }

    let result = result.await;

    match result {
        Ok(_) => {
            app.sender
                .send(outgo::Msg::Exited {
                    code: 0,
                    data: Box::from(""),
                })
                .await?
        }
        Err(e) => {
            log::error!("error: '{e}'");
            app.sender
                .send(outgo::Msg::Exited {
                    code: 1,
                    data: e.to_string().into_boxed_str(),
                })
                .await?
        }
    }

    Ok(())
}
