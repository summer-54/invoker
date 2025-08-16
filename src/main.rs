mod api;
mod judge;
mod pull;
mod sandboxes;
mod ws;

use std::{str::FromStr, sync::Arc};

use futures::StreamExt;
use tokio::{
    io::AsyncRead,
    sync::{Mutex, mpsc::unbounded_channel},
};
use ws::Uri;

pub use anyhow::Result;
pub use env_logger;

use tokio_tar::Archive;

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
                        let package = Archive::new(&*data);
                        let (sender, mut receiver) = unbounded_channel::<judge::TestResult>();
                        let self_clone = Arc::clone(&self);
                        let handler = tokio::spawn(async move {
                            while let Some(test_result) = receiver.recv().await {
                                self_clone
                                    .ws
                                    .send(api::outgo::Msg::TestVerdict {
                                        test_id: test_result.id,
                                        verdict: test_result.verdict,
                                        data: "".to_string().into_boxed_str(), // TODO: data
                                    })
                                    .await
                                    .expect("webscoket not working unexpectually");
                            }
                        });
                        let result = self.judger.judge(package, sender).await;
                        handler.await?;
                    }
                    api::income::Msg::Stop => self.judger.stop_all().await?,
                    api::income::Msg::Close => break,
                }
            }
            log::info!("invoker was closed ws message");
            Result::<()>::Ok(())
        });

        Ok(())
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
