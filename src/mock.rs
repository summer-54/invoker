use crate::prelude::*;

use tokio::sync::{
    Mutex,
    mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel},
};

use crate::server::stream::{
    AuthIncome, AuthOutgo, LoadIncome, LoadOutgo, MasterIncome, MasterOutgo, Stream,
};

pub struct AuthStream;

impl Stream<AuthIncome, AuthOutgo> for AuthStream {
    async fn recv(&self) -> Result<AuthIncome> {
        futures::future::pending().await
    }
    async fn send(&self, msg: AuthOutgo) -> Result<()> {
        log::info!("send into mock AUTH stream: {msg:?}");
        Ok(())
    }
}

pub struct MasterStream {
    receiver: Mutex<UnboundedReceiver<MasterIncome>>,
}

impl Stream<MasterIncome, MasterOutgo> for MasterStream {
    async fn recv(&self) -> Result<MasterIncome> {
        let Some(message) = self.receiver.lock().await.recv().await else {
            futures::future::pending::<()>().await;
            unreachable!();
        };
        Ok(message)
    }
    async fn send(&self, msg: MasterOutgo) -> Result<()> {
        log::info!("send into mock MASTER stream: {msg:?}");
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

impl Stream<LoadIncome, LoadOutgo> for LoadStream {
    async fn recv(&self) -> Result<LoadIncome> {
        let Some(message) = self.receiver.lock().await.recv().await else {
            futures::future::pending::<()>().await;
            unreachable!()
        };
        Ok(message)
    }
    async fn send(&self, msg: LoadOutgo) -> Result<()> {
        log::info!("send into mock LOAD stream: {msg:?}");
        match msg {
            LoadOutgo::Load { package_id } => {
                let sender = self.sender.clone();
                let path = self.dir.join(package_id.to_string());
                tokio::spawn(async move {
                    let data = match tokio::fs::read(&path).await {
                        Ok(data) => data.into(),
                        Err(error) => {
                            log::error!(
                                "mock loader can't found package: {}, {}",
                                path.display(),
                                error
                            );
                            return;
                        }
                    };
                    if let Err(error) = sender.send(LoadIncome::Package(data)) {
                        log::error!("error while sending message: {error}");
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
