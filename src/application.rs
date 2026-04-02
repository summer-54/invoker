use crate::prelude::*;

use std::sync::Arc;

use invoker_auth::{Cert, Challenge, policy};
use tokio::{sync::mpsc::unbounded_channel, task::JoinHandle};

use crate::{
    Result, judge,
    server::{
        self,
        stream::{AuthStream, MasterStream, master::FullVerdict},
    },
};
use tar_archive_rs::{self as archive, ArchiveItem};

pub struct App {
    pub master_stream: MasterStream,
    pub auth_stream: AuthStream,
    pub judge_service: Arc<judge::Service>,
    pub cert: Arc<Cert>,
}

impl App {
    pub fn start_judgment(
        self: &Arc<Self>,
        data: Box<[u8]>,
    ) -> JoinHandle<crate::Result<judge::api::submission::Result>> {
        use server::stream::MasterOutgo as Outgo;
        let self_clone = Arc::clone(&self);
        let (sender, mut receiver) = unbounded_channel::<(usize, judge::api::test::Result)>();
        let handler = tokio::spawn(async move {
            while let Some((id, test_result)) = receiver.recv().await {
                let data = archive::pack(&[
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
                    log::error!("sending 'TestVerdict': compression error: {e:?}");
                    vec![].into_boxed_slice()
                });
                self_clone
                    .master_stream
                    .send(Outgo::TestVerdict {
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
            let package = archive::Archive::new(&*data);
            let result = Arc::clone(&self_clone.judge_service)
                .judge(package, sender)
                .await;
            _ = handler.await;
            match &result {
                Ok(full_verdict) => self_clone
                    .master_stream
                    .send(Outgo::FullVerdict(match full_verdict {
                        judge::api::submission::Result::Ok {
                            score,
                            groups_score,
                        } => FullVerdict::Ok {
                            score: *score,
                            groups_score: groups_score.clone(),
                        },
                        judge::api::submission::Result::Ce(msg) => FullVerdict::Ce(msg.clone()),
                        judge::api::submission::Result::Te(msg) => FullVerdict::Te(msg.clone()),
                    }))
                    .await
                    .map_err(|e| {
                        log::error!("sending message error: {e:?}");
                    })
                    .expect("message sending error"),
                Err(e) => {
                    log::error!("judger error: {e:?}");
                    self_clone
                        .master_stream
                        .send(Outgo::Error {
                            msg: e.to_string().into_boxed_str(),
                        })
                        .await
                        .unwrap();
                }
            }
            result
        })
    }

    async fn solve_challenge(&self, challenge: Challenge) -> Result<()> {
        use server::stream::AuthOutgo as Outgo;
        let solution = challenge.solve(&*self.cert, &policy::StandardPolicy::new())?;
        self.auth_stream
            .send(Outgo::ChallengeSolution(solution))
            .await
    }

    async fn listen_master_stream(self: Arc<Self>) -> Result<()> {
        use server::stream::master::Income;
        log::info!("master channel listener open");
        loop {
            let msg = self
                .master_stream
                .recv()
                .await
                .context("reading master message")?;
            match msg {
                Income::Start { data } => _ = self.start_judgment(data),
                Income::Stop => self
                    .judge_service
                    .cancel_all_tests()
                    .await
                    .context("all tests cancelling")?,
                Income::Close => break,
            }
        }
        log::info!("message listner close");
        Result::<()>::Ok(())
    }

    async fn listen_auth_stream(self: Arc<Self>) -> Result<()> {
        use server::stream::auth::Income;
        log::info!("auth channel listener open");
        loop {
            let msg = self
                .auth_stream
                .recv()
                .await
                .context("reading auth message")?;
            match msg {
                Income::AuthVerdict(verdict) => {
                    if !verdict {
                        bail!("auth FAILED");
                    }
                }
                Income::Challenge(challenge) => (&*self)
                    .solve_challenge(challenge)
                    .await
                    .context("solving auth challenge")?,
            }
        }
    }

    pub async fn run(self: &Arc<Self>) -> Result<()> {
        tokio::select! {
            res = tokio::spawn(Arc::clone(self).listen_master_stream()) => res?,
            res = tokio::spawn(Arc::clone(self).listen_auth_stream()) => res?,
        }
    }
}
