mod api;
mod archive;
mod judge;
mod pull;
mod sandboxes;
mod ws;

use tokio::io::{AsyncReadExt, stdin};

use crate::{api::outgo::FullVerdict, archive::ArchiveItem};

pub use {
    anyhow::{Error, Result, anyhow},
    env_logger,
};

use std::{str::FromStr, sync::Arc};
use {tokio::sync::mpsc::unbounded_channel, ws::Uri};

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

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let task_manager_host = std::env::var("TASK_MANAGER_HOST")
        .expect("enviroment variable 'TASK_MANAGER_HOST' not found");

    let task_manager_uri = Uri::from_str(
        std::env::var("TASK_MANAGER_WS_URI")
            .expect("enviroment variable 'TASK_MANAGER_WS_URI' not found")
            .as_str(),
    )?;

    let config_dir = std::env::var("INVOKER_CONFIG_DIR")
        .expect("enviroment variable 'INVOKER_CONFIG_DIR' not found");

    let work_dir = std::env::var("INVOKER_WORK_DIR")
        .expect("enviroment variable 'INVOKER_WORK_DIR' not found");

    log::info!("task manager uri: {task_manager_uri}");
    log::info!("invoker work dir: {work_dir}");
    log::warn!("{work_dir} can be cleared. Press any key to continue ...");

    let mut _buf = vec![];
    stdin().read_buf(&mut _buf).await?;

    let ws_client = ws::Service::from_uri(task_manager_host, task_manager_uri).await?;
    let isolate_client = sandboxes::isolate::Service::new(&config_dir).await?;

    let app = App {
        ws: ws_client,
        judger: Arc::new(
            judge::Service::new(isolate_client, format!("{work_dir}/judge").into_boxed_str()).await,
        ),
    };

    Arc::new(app).run().await?;

    Ok(())
}
