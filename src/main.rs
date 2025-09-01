mod api;
mod archive;
mod judge;
mod pull;
mod sandboxes;
mod ws;

use crate::{
    api::outgo::{self, FullVerdict},
    archive::ArchiveItem,
};

pub use {
    anyhow::{Error, Result, anyhow},
    env_logger,
    serde::{Deserialize, Serialize},
};

use {
    std::{str::FromStr, sync::Arc},
    tokio::sync::mpsc::unbounded_channel,
    uuid::Uuid,
    ws::Uri,
};

struct App {
    pub ws: ws::Service,
    pub judger: Arc<judge::Service>,
}

impl App {
    pub async fn run(self: Arc<Self>) -> Result<()> {
        tokio::spawn(async move {
            loop {
                log::info!("waiting for message...");
                let msg = self.ws.recv().await?;
                match msg {
                    api::income::Msg::Start { data } => {
                        let self_clone = Arc::clone(&self);
                        let (sender, mut receiver) =
                            unbounded_channel::<(usize, judge::TestResult)>();
                        let handler = tokio::spawn(async move {
                            while let Some((id, test_result)) = receiver.recv().await {
                                let data = archive::compress(&[
                                    ArchiveItem {
                                        path: "output",
                                        data: test_result.output.as_bytes(),
                                    },
                                    ArchiveItem {
                                        path: "message",
                                        data: test_result.message.as_bytes(),
                                    },
                                ])
                                .await
                                .unwrap_or_else(|e| {
                                    log::error!("sennding 'TestVerdict': compression error: {e}");
                                    vec![].into_boxed_slice()
                                });
                                self_clone
                                    .ws
                                    .send(api::outgo::Msg::TestVerdict {
                                        test_id: id,
                                        verdict: test_result.verdict,
                                        time: test_result.time,
                                        memory: test_result.memory,
                                        data,
                                    })
                                    .await
                                    .expect("webscoket isn't working unexpexted");
                            }
                        });
                        let self_clone = Arc::clone(&self);

                        tokio::spawn(async move {
                            let package = archive::decompress(&*data).await;
                            let result =
                                Arc::clone(&self_clone.judger).judge(package, sender).await;
                            _ = handler.await;
                            match result {
                                Ok(full_verdict) => self_clone
                                    .ws
                                    .send(api::outgo::Msg::FullVerdict(match full_verdict {
                                        judge::FullResult::Ok {
                                            score,
                                            groups_score,
                                        } => FullVerdict::Ok {
                                            score,
                                            groups_score,
                                        },
                                        judge::FullResult::Ce(msg) => FullVerdict::Ce(msg),
                                        judge::FullResult::Te(msg) => FullVerdict::Te(msg),
                                    }))
                                    .await
                                    .map_err(|e| {
                                        log::error!("sending message error: {e:?}");
                                    })
                                    .expect("message sending error"),
                                Err(error) => {
                                    log::error!("judger error: {error}");
                                    self_clone
                                        .ws
                                        .send(api::outgo::Msg::Error {
                                            msg: error.to_string().into_boxed_str(),
                                        })
                                        .await
                                        .unwrap();
                                }
                            }
                        });
                    }
                    api::income::Msg::Stop => self.judger.stop_all().await?,
                    api::income::Msg::Close => break,
                }
            }
            log::info!("message listner close close");
            Result::<()>::Ok(())
        })
        .await?
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "UPPERCASE")]
struct Config {
    pub manager_host: Box<str>,
    pub config_dir: Box<str>,
    pub work_dir: Box<str>,

    pub isolate_exe_path: Box<str>,
}

impl Config {
    pub async fn init() -> Result<Self> {
        let config = envy::prefixed("INVOKER_").from_env::<Config>()?;
        log::info!("enviroment variables: {config:#?}");
        Ok(config)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let config = Config::init().await?;

    let judger_work_dir = format!("{}/judge", config.work_dir).into_boxed_str();
    let token = Uuid::new_v4();
    println!("invoker token: {token}");

    let isolate_client =
        sandboxes::isolate::Service::new(&config.config_dir, config.isolate_exe_path).await?;

    let ws_client = ws::Service::new(
        config.manager_host.as_ref(),
        Uri::from_str(format!("ws://{}", config.manager_host).as_str())?,
    )
    .await?;

    ws_client.send(api::outgo::Msg::Token(token)).await?;

    let app = App {
        ws: ws_client,
        judger: Arc::new(judge::Service::new(isolate_client, judger_work_dir).await),
    };

    let app = Arc::new(app);
    let result = Arc::clone(&app).run().await;

    match result {
        Ok(_) => {
            app.ws
                .send(outgo::Msg::Exited {
                    code: 0,
                    data: Box::from(""),
                })
                .await?
        }
        Err(e) => {
            log::error!("error: '{e}'");
            app.ws
                .send(outgo::Msg::Exited {
                    code: 1,
                    data: e.to_string().into_boxed_str(),
                })
                .await?
        }
    }

    Ok(())
}
