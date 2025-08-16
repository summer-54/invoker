mod api;
mod archive;
mod judge;
mod pull;
mod sandboxes;
mod ws;

pub use {
    anyhow::{Result, anyhow},
    env_logger,
};

use std::{str::FromStr, sync::Arc};
use {tokio::sync::mpsc::unbounded_channel, tokio_tar::Archive, ws::Uri};

struct App {
    pub ws: ws::Service,
    pub judger: judge::Service,
}

impl App {
    pub async fn run(self: Arc<Self>) -> Result<()> {
        tokio::spawn(async move {
            loop {
                let msg = self.ws.recv().await?;
                match msg {
                    api::income::Msg::Start { data } => {
                        let self_clone = Arc::clone(&self);
                        let (sender, mut receiver) = unbounded_channel::<judge::TestResult>();
                        let handler = tokio::spawn(async move {
                            while let Some(test_result) = receiver.recv().await {
                                let data = archive::compress::<&[u8]>(&[
                                    ("output", test_result.output.as_bytes()),
                                    ("checker_output", test_result.checker_output.as_bytes()),
                                ])
                                .await
                                .unwrap_or_else(|e| {
                                    log::error!("file compressing error: {e}");
                                    vec![].into_boxed_slice()
                                });
                                self_clone
                                    .ws
                                    .send(api::outgo::Msg::TestVerdict {
                                        test_id: test_result.id,
                                        verdict: test_result.verdict,
                                        time: test_result.time,
                                        memory: test_result.memory,
                                        data, // TODO: data
                                    })
                                    .await
                                    .expect("webscoket isn't working unexpexted");
                            }
                        });
                        let self_clone = Arc::clone(&self);

                        tokio::spawn(async move {
                            let package = Archive::new(&*data);
                            let result = self_clone.judger.judge(package, sender).await;
                            _ = handler.await;
                            match result {
                                Ok(full_verdict) => self_clone
                                    .ws
                                    .send(api::outgo::Msg::FullVerdict {
                                        score: full_verdict.score,
                                        groups_score: full_verdict.groups_score,
                                    })
                                    .await
                                    .expect("websocket isn't working unexpected"),
                                Err(error) => {
                                    log::error!("judger error:\n{error}");
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
            log::info!("invoker was closed ws message");
            Result::<()>::Ok(())
        })
        .await?
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let task_manager_uri = Uri::from_str(
        std::env::var("TASK_MANAGER_WS_URI")
            .expect("enviroment variable 'TASK_MANAGER_WS_URI' not found")
            .as_str(),
    )?;

    let config_dir = std::env::var("INVOKER_CONFIG_DIR")
        .expect("enviroment variable 'INVOKER_CONFIG_DIR' not found");

    log::info!("{} ", task_manager_uri);

    let ws_client = ws::Service::from_uri(task_manager_uri).await?;
    let isolate_client = sandboxes::isolate::Service::new(&config_dir).await?;

    let app = App {
        ws: ws_client,
        judger: judge::Service::new(isolate_client),
    };

    Arc::new(app).run().await?;

    Ok(())
}
