mod application;
mod channel;
mod judge;
mod logger;
mod sandbox;
mod server;

use anyhow::Context;
use colored::Colorize;
use invoker_auth::{Cert, Parse};

use crate::{
    application::App,
    server::{
        income,
        outgo::{self, Sender},
    },
};

#[cfg(not(feature = "mock"))]
use {crate::server::websocket::Uri, std::str::FromStr};
pub use {
    anyhow::{Error, Result, anyhow},
    env_logger,
    logger::LogState,
    serde::{Deserialize, Serialize},
};

use {std::sync::Arc, uuid::Uuid};

#[derive(Clone, Deserialize, Debug)]
struct Config {
    #[cfg(not(feature = "mock"))]
    pub manager_host: Box<str>,
    pub config_dir: Box<str>,
    pub work_dir: Box<str>,

    pub isolate_exe_path: Box<str>,
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

#[cfg(not(feature = "mock"))]
async fn init_communnication(
    token: Uuid,
    config: Config,
) -> Result<(Arc<impl income::Receiver>, Arc<impl outgo::Sender>)> {
    let websocket_service = Arc::new(
        server::websocket::Service::new(
            config.manager_host.as_ref(),
            Uri::from_str(format!("ws://{}", config.manager_host).as_str())?,
        )
        .await?,
    );

    websocket_service
        .send(server::outgo::Msg::Token {
            token,
            name: config.cert_name,
        })
        .await?;

    Ok((websocket_service.clone(), websocket_service))
}

#[cfg(feature = "mock")]
async fn init_communnication(
    _token: Uuid,
    _config: Config,
) -> Result<(Arc<impl income::Receiver>, Arc<impl outgo::Sender>)> {
    log::info!("{} communication initialized", "mock".bold());
    Ok((Arc::new(income::MockReceiver), Arc::new(outgo::MockSender)))
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    if !nix::unistd::Uid::current().is_root() {
        println!("{}", "must started as root".red().bold());
        return Err(anyhow!("must started as root"));
    }

    let config = Config::init().await?;

    let judger_work_dir = format!("{}/judge", config.work_dir).into_boxed_str();
    let token = Uuid::new_v4();
    println!("\n[{}] invoker token\n", format!("{token}").yellow().bold());

    let (receiver, sender) = init_communnication(token, config.clone()).await?;
    let cert = Cert::from_file(&*config.cert_path)?;
    let isolate_service =
        sandbox::Service::new(&config.config_dir, config.isolate_exe_path).await?;

    let app = App {
        receiver,
        sender,
        judge_service: Arc::new(
            judge::Service::new(&config.config_dir, isolate_service, judger_work_dir).await,
        ),
        cert: Arc::new(cert),
    };

    let app = Arc::new(app);
    let result = Arc::clone(&app).run();
    for name in std::env::args().skip(1) {
        app.start_judgment(
            tokio::fs::read(name.as_str())
                .await
                .context("reading file '{name}'")?
                .into_boxed_slice(),
        );
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
