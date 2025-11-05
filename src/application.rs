use std::sync::Arc;

use tokio::{sync::mpsc::unbounded_channel, task::JoinHandle};

use crate::{
    Result,
    judge::{self, FullResult},
    server::{
        self, income,
        outgo::{self, FullVerdict},
    },
};
use archive::{self, ArchiveItem};

pub struct App<S: outgo::Sender, R: income::Receiver> {
    pub sender: Arc<S>,
    pub receiver: Arc<R>,
    pub judge_service: Arc<judge::Service>,
}

impl<S: outgo::Sender + Send + Sync + 'static, R: income::Receiver + Send + 'static> App<S, R> {
    pub fn start_judgment(
        self: &Arc<Self>,
        data: Box<[u8]>,
    ) -> JoinHandle<crate::Result<FullResult>> {
        let self_clone = Arc::clone(&self);
        let (sender, mut receiver) = unbounded_channel::<(usize, judge::TestResult)>();
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
                    log::error!("sending 'TestVerdict': compression error: {e}");
                    vec![].into_boxed_slice()
                });
                self_clone
                    .sender
                    .send(server::outgo::Msg::TestVerdict {
                        test_id: id,
                        verdict: test_result.verdict,
                        time: test_result.time,
                        memory: test_result.memory,
                        data,
                    })
                    .await
                    .expect("websocket closed unexpectedly");
            }
        });
        let self_clone = Arc::clone(&self);

        tokio::spawn(async move {
            let package = archive::decompress(&*data).await;
            let result = Arc::clone(&self_clone.judge_service)
                .judge(package, sender)
                .await;
            _ = handler.await;
            match &result {
                Ok(full_verdict) => self_clone
                    .sender
                    .send(server::outgo::Msg::FullVerdict(match full_verdict {
                        judge::FullResult::Ok {
                            score,
                            groups_score,
                        } => FullVerdict::Ok {
                            score: *score,
                            groups_score: groups_score.clone(),
                        },
                        judge::FullResult::Ce(msg) => FullVerdict::Ce(msg.clone()),
                        judge::FullResult::Te(msg) => FullVerdict::Te(msg.clone()),
                    }))
                    .await
                    .map_err(|e| {
                        log::error!("sending message error: {e:?}");
                    })
                    .expect("message sending error"),
                Err(e) => {
                    log::error!("judger error: {e:?}");
                    self_clone
                        .sender
                        .send(server::outgo::Msg::Error {
                            msg: e.to_string().into_boxed_str(),
                        })
                        .await
                        .unwrap();
                }
            }
            result
        })
    }

    pub async fn run(self: Arc<Self>) -> Result<()> {
        loop {
            log::info!("message listner open");
            let msg = self.receiver.recv().await?;
            match msg {
                server::income::Msg::Start { data } => _ = self.start_judgment(data),
                server::income::Msg::Stop => self.judge_service.cancel_all_tests().await?,
                server::income::Msg::Close => break,
            }
        }
        log::info!("message listner close close");
        Result::<()>::Ok(())
    }
}
