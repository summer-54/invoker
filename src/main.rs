mod api;
mod judge;
mod pull;
mod sandboxes;
mod ws;

use std::{str::FromStr, sync::Arc};

use ws::Uri;

pub use anyhow::Result;
pub use env_logger;

struct App {
    ws: ws::Service,
    isolate: Arc<sandboxes::isolate::Service>,
}

impl App {
    pub async fn run(self: Arc<Self>) -> Result<()> {
        tokio::spawn(async move {
            loop {
                let msg = self.ws.recv().await?;
                match msg {
                    api::income::Msg::Start { data } => {
                        judge::judge(Arc::clone(&self), todo!("path to packeage with task"))?
                    }
                    api::income::Msg::Stop => judge::stop_all(Arc::clone(&self))?,
                }
            }
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
        isolate: isolate_client,
    };

    Arc::new(app).run().await?;

    Ok(())
}
