use crate::prelude::*;

use tokio::sync::{
    Mutex,
    mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel},
};

use crate::{
    logger::LogState,
    server::stream::{
        AuthIncome, AuthOutgo, LoadIncome, LoadOutgo, MasterIncome, MasterOutgo, Stream,
    },
};

async fn eternal() -> ! {
    futures::future::pending().await
}

pub struct AuthStream;
impl AuthStream {
    fn log_state() -> std::sync::Arc<LogState> {
        LogState::new().push("stream", "mock::auth")
    }
}

impl Stream<AuthIncome, AuthOutgo> for AuthStream {
    async fn recv(&self) -> Result<AuthIncome> {
        let log_state = Self::log_state();
        log::trace!("{log_state} waiting message... (eternal)");
        eternal().await
    }
    async fn send(&self, msg: AuthOutgo) -> Result<()> {
        let log_state = Self::log_state();
        log::info!("{log_state} <- {msg:?}");
        Ok(())
    }
}

pub struct MasterStream {
    receiver: Mutex<UnboundedReceiver<MasterIncome>>,
}

impl MasterStream {
    fn log_state() -> std::sync::Arc<LogState> {
        LogState::new().push("stream", "mock::master")
    }
}

impl Stream<MasterIncome, MasterOutgo> for MasterStream {
    async fn recv(&self) -> Result<MasterIncome> {
        let log_state = Self::log_state();
        log::trace!("{log_state} waiting message...");
        let Some(msg) = self.receiver.lock().await.recv().await else {
            log::trace!("{log_state} receiver closed");
            eternal().await
        };
        log::info!("{log_state} -> {msg:?}");
        Ok(msg)
    }
    async fn send(&self, msg: MasterOutgo) -> Result<()> {
        let log_state = Self::log_state();
        log::info!("{log_state} <- {msg:?}");
        Ok(())
    }
}

impl MasterStream {
    pub fn new() -> (UnboundedSender<MasterIncome>, MasterStream) {
        let (sender, receiver) = unbounded_channel();
        (
            sender,
            MasterStream {
                receiver: Mutex::new(receiver),
            },
        )
    }
}

pub struct LoadStream {
    receiver: Mutex<UnboundedReceiver<LoadIncome>>,
    sender: UnboundedSender<LoadIncome>,
    dir: Box<Path>,
}

impl LoadStream {
    fn log_state() -> std::sync::Arc<LogState> {
        LogState::new().push("stream", "mock::load")
    }
}

impl Stream<LoadIncome, LoadOutgo> for LoadStream {
    async fn recv(&self) -> Result<LoadIncome> {
        let log_state = Self::log_state();
        log::trace!("{log_state} waiting message...");
        let Some(msg) = self.receiver.lock().await.recv().await else {
            log::trace!("{log_state} receiver closed");
            eternal().await
        };
        log::info!("{log_state} -> {msg:?}");
        Ok(msg)
    }
    async fn send(&self, msg: LoadOutgo) -> Result<()> {
        let log_state = Self::log_state();
        log::info!("{log_state} <- {msg:?}");
        match msg {
            LoadOutgo::Load { package_id } => {
                let sender = self.sender.clone();
                let path = self.dir.join(package_id.to_string());
                tokio::spawn(async move {
                    let data = match tokio::fs::read(&path).await {
                        Ok(data) => data.into(),
                        Err(error) => {
                            log::error!("{error}");
                            log::error!(
                                "{log_state} package {} not found in {}",
                                package_id.to_string().bold(),
                                path.display().to_string().bright_white(),
                            );
                            return;
                        }
                    };
                    if let Err(error) = sender.send(LoadIncome::Package(data)) {
                        log::error!("{log_state} internal sender error: {error}");
                    }
                });
            }
        }
        Ok(())
    }
}

impl LoadStream {
    pub fn new(dir: impl AsRef<Path>) -> Self {
        let (sender, receiver) = unbounded_channel();
        Self {
            dir: dir.as_ref().into(),
            sender,
            receiver: Mutex::new(receiver),
        }
    }
}
