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
use uuid::Uuid;

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
    pub token: Box<str>,

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
) -> Result<
    Communication<
        impl Stream<AuthIncome, AuthOutgo>,
        impl Stream<MasterIncome, MasterOutgo>,
        impl Stream<JudgeIncome, JudgeOutgo>,
    >,
> {
    use tonic::transport::Channel;

    let channel = Channel::builder(
        config
            .manager_host
            .parse()
            .context("parsing manager host field")?,
    )
    .connect()
    .await?;

    let mut client = grpc::Client::with_interceptor(
        channel,
        server::TokenInterceptor::new(&config.token).context("TokenInterceptor creating")?,
    );
    let auth_stream = {
        use tonic::Request;

        let (sender_outgo, receiver_outgo) = tokio::sync::mpsc::unbounded_channel();
        let request = Request::new(tokio_stream::wrappers::UnboundedReceiverStream::new(
            receiver_outgo,
        ));
        let response = client
            .auth(request)
            .await
            .context("gRPC: auth()")?
            .into_inner();
        grpc::Stream::new(response, sender_outgo)
    };

    let master_stream = {
        use tonic::Request;
        let (sender_outgo, receiver_outgo) = tokio::sync::mpsc::unbounded_channel();
        let request = Request::new(tokio_stream::wrappers::UnboundedReceiverStream::new(
            receiver_outgo,
        ));
        let response = client
            .master_stream(request)
            .await
            .context("gRPC: auth()")?
            .into_inner();
        grpc::Stream::new(response, sender_outgo)
    };

    let judge_stream = {
        use tonic::Request;
        let (sender_outgo, receiver_outgo) = tokio::sync::mpsc::unbounded_channel();
        let request = Request::new(tokio_stream::wrappers::UnboundedReceiverStream::new(
            receiver_outgo,
        ));
        let response = client
            .judge_stream(request)
            .await
            .context("gRPC: auth()")?
            .into_inner();
        grpc::Stream::new(response, sender_outgo)
    };
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
    let token = Uuid::new_v4();
    println!("\n[{}] invoker token\n", format!("{token}").yellow().bold());

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

    Stream::<AuthIncome, AuthOutgo>::send(
        &app.auth_stream,
        AuthOutgo::CertName(config.cert_name.clone()),
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
