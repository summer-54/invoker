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

use logger::LogState;
use serde::Deserialize;
#[cfg(not(feature = "mock"))]
use toaster_lib_rs::{
    auth::{CertName, Token},
    server::grpc::ClientStream,
};
#[cfg(not(feature = "mock"))]
use tonic::service::Interceptor;

use std::{path::Path, sync::Arc};
use toaster_lib_rs::{
    auth::{Cert, Parse},
    logger,
    server::grpc,
};

use crate::{
    application::App,
    server::stream::{
        AuthIncome, AuthOutgo, JudgeIncome, JudgeOutgo, MasterIncome, MasterOutgo, Stream,
    },
};

#[cfg(not(feature = "mock"))]
#[derive(Clone, Deserialize, Debug)]
struct Config {
    pub token: Token,

    #[cfg(not(feature = "mock"))]
    pub manager_host: Box<str>,
    pub config_dir: Box<Path>,
    pub work_dir: Box<Path>,

    pub cert_name: CertName,
    pub cert_path: Box<Path>,
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
) -> Result<
    Communication<
        impl Stream<AuthIncome, AuthOutgo>,
        impl Stream<MasterIncome, MasterOutgo>,
        impl Stream<JudgeIncome, JudgeOutgo>,
    >,
> {
    use tonic::{codec::CompressionEncoding, transport::Channel};
    let uri: http::Uri = config
        .manager_host
        .parse()
        .context("parsing manager host field")?;

    log::trace!("manager host uri: {uri:?}");

    let channel = Channel::builder(uri).connect().await?;
    let mut token_interceptor =
        server::TokenInterceptor::new(&config.token).context("TokenInterceptor creating")?;
    let mut cert_name_interceptor =
        server::CertNameInterceptor::new(&config.cert_name).context("TokenInterceptor creating")?;
    let mut client = grpc::invoker_manager::Client::with_interceptor(channel, |req| {
        cert_name_interceptor.call(token_interceptor.call(req)?)
    })
    .accept_compressed(CompressionEncoding::Zstd)
    .send_compressed(CompressionEncoding::Zstd)
    .max_decoding_message_size(1024 * 1024 * 1024)
    .max_encoding_message_size(1024 * 1024 * 1024);
    let auth_stream = ClientStream::from_fn(async |req| client.auth(req).await).await?;
    let master_stream = ClientStream::from_fn(async |req| client.master_stream(req).await).await?;
    let judge_stream = ClientStream::from_fn(async |req| client.judge_stream(req).await).await?;
    let communication = Communication {
        auth_stream,
        master_stream,
        judge_stream,
    };

    Ok(communication)
}

#[cfg(feature = "mock")]
async fn init_mock_communication() -> (
    tokio::sync::mpsc::UnboundedSender<JudgeIncome>,
    Communication<
        impl Stream<AuthIncome, AuthOutgo>,
        impl Stream<MasterIncome, MasterOutgo>,
        impl Stream<JudgeIncome, JudgeOutgo>,
    >,
) {
    use crate::mock::{JudgeStream, Mock, auth_log, master_log};

    log::info!("{} communication initialized", "mock".bold());
    let (sender, judge_stream) = JudgeStream::new();
    // let load_stream = LoadStream::new(&config.loader_dir);
    (
        sender,
        Communication {
            auth_stream: Mock::new(auth_log),
            master_stream: Mock::new(master_log),
            judge_stream,
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
    let token = config.token.clone();
    println!(
        "\n[{}] invoker token\n",
        format!("{token:?}").yellow().bold()
    );

    #[cfg(not(feature = "mock"))]
    let communication = init_websocket_communication(config.clone()).await?;

    #[cfg(feature = "mock")]
    let communication = {
        let mut args = std::env::args().skip(1);
        let (master_sender, communication) = init_mock_communication().await;
        while let Some(name) = args.next() {
            let Some(lang) = args.next() else {
                bail!("lang was not founded")
            };
            let lang = judge::api::Lang::try_from(&*lang)?;

            let data = tokio::fs::read(&name)
                .await
                .context(format!("reading file '{name}'"))?
                .into_boxed_slice();
            master_sender.send(JudgeIncome::Run { lang, data })?;
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
