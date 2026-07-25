use crate::prelude::*;

use tokio::sync::{
    Mutex,
    mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel},
};

use crate::{
    logger::LogState,
    server::stream::{AuthOutgo, JudgeIncome, JudgeOutgo, MasterOutgo, Stream},
};

pub use toaster_lib_rs::server::mock::Mock;

async fn eternal() -> ! {
    futures::future::pending().await
}

pub fn auth_log(msg: AuthOutgo) {
    let log_state = LogState::new().push("stream", "mock::auth");
    log::info!("{log_state} <- {msg:?}");
}

pub fn master_log(msg: MasterOutgo) {
    let log_state = LogState::new().push("stream", "mock::master");
    log::info!("{log_state} <- {msg:?}");
}

pub struct JudgeStream {
    receiver: Mutex<UnboundedReceiver<JudgeIncome>>,
}

impl JudgeStream {
    fn log_state() -> std::sync::Arc<LogState> {
        LogState::new().push("stream", "mock::judge")
    }
}

impl Stream<JudgeIncome, JudgeOutgo> for JudgeStream {
    async fn recv(&self) -> Result<JudgeIncome> {
        let log_state = Self::log_state();
        log::trace!("{log_state} waiting message...");
        let Some(msg) = self.receiver.lock().await.recv().await else {
            log::trace!("{log_state} receiver closed");
            eternal().await
        };
        log::info!("{log_state} -> {msg:?}");
        Ok(msg)
    }
    async fn send(&self, msg: JudgeOutgo) -> Result<()> {
        let log_state = Self::log_state();
        log::info!("{log_state} <- {msg:?}");
        Ok(())
    }
}

impl JudgeStream {
    pub fn new() -> (UnboundedSender<JudgeIncome>, JudgeStream) {
        let (sender, receiver) = unbounded_channel();
        (
            sender,
            JudgeStream {
                receiver: Mutex::new(receiver),
            },
        )
    }
}
