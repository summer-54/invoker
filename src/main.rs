mod application;
mod channel;
mod consts;
mod file;
mod judge;
mod logger;
mod prelude;
mod sandbox;
mod server;

use invoker_auth::{Cert, Parse};
use prelude::*;

const VISIBLE_DATA_LEN: usize = 30;

fn short_slice_u8(data: &[u8]) -> &[u8] {
    &data[..std::cmp::min(data.len(), VISIBLE_DATA_LEN)]
}

#[cfg(not(feature = "mock"))]
use crate::server::websocket;
use crate::{
    application::App,
    server::stream::{AuthIncome, AuthOutgo, MasterIncome, MasterOutgo},
};
use std::path::Path;

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

#[cfg(not(feature = "mock"))]
async fn init_communnication(config: Config) -> Result<Arc<websocket::Channel>> {
    Ok(Arc::new(
        server::websocket::Channel::new(
            config.manager_host.as_ref(),
            Uri::from_str(format!("ws://{}", config.manager_host).as_str())?,
        )
        .await?,
    ))
}

#[cfg(feature = "mock")]
async fn init_communnication(_config: Config) -> Result<Arc<impl MultiplexChannel>> {
    log::info!("{} communication initialized", "mock".bold());
    Ok((Arc::new(server::mock::MockChannel::new())))
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    if !nix::unistd::Uid::current().is_root() {
        println!("{}", "must started as root".red().bold());
        bail!("must started as root");
    }

    let config = Config::init().await?;

    if !tokio::fs::try_exists(&*config.config_dir).await? {
        log::error!("config directory: '{:?}' not founded", config.config_dir);
        bail!("config directory: '{:?}' not founded", config.config_dir);
    }

    let judger_work_dir = config.work_dir.join("judge");
    let token = Uuid::new_v4();
    println!("\n[{}] invoker token\n", format!("{token}").yellow().bold());

    let channel = init_communnication(config.clone()).await?;
    let cert = Cert::from_file(&*config.cert_path)?;
    let isolate_service =
        sandbox::Service::new(&config.config_dir, config.isolate_exe_path).await?;

    let inner_provider = file::stream_provider::StreamProvider::new(
        channel.new_stream(consts::streams_names::LOAD).await,
    );
    let file_provider = file::CachingProvider::init(config.cache_dir, inner_provider).await?;

    let app = App {
        master_stream: channel
            .new_stream::<MasterIncome, MasterOutgo>(consts::streams_names::MASTER)
            .await,
        auth_stream: channel
            .new_stream::<AuthIncome, AuthOutgo>(consts::streams_names::AUTH)
            .await,
        judge_service: Arc::new(
            judge::Service::new(&config.config_dir, isolate_service, judger_work_dir).await,
        ),
        cert: Arc::new(cert),
        file_provider,
    };

    app.master_stream
        .send(MasterOutgo::Token {
            token,
            name: config.cert_name,
        })
        .await?;

    tokio::spawn(channel.run());

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
            app.master_stream
                .send(MasterOutgo::Exited {
                    code: 0,
                    data: Box::from(""),
                })
                .await?
        }
        Err(e) => {
            log::error!("error: '{e:?}'");
            app.master_stream
                .send(MasterOutgo::Exited {
                    code: 1,
                    data: format!("{e:?}").into_boxed_str(),
                })
                .await?
        }
    }
    Ok(())
}
