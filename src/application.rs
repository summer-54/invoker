use crate::prelude::*;

use invoker_auth::{Cert, Challenge, policy};
use tar_archive_rs::{self as archive, ArchiveItem};
use tokio::{sync::mpsc::unbounded_channel, task::JoinHandle};

use std::sync::Arc;

use crate::{
    Result, file,
    judge::{self, api::Lang},
    server::{
        self,
        stream::{AuthIncome, AuthOutgo, MasterIncome, MasterOutgo, Stream, master::FullVerdict},
    },
};

pub struct App<
    P: file::providers::Provider,
    AS: Stream<AuthIncome, AuthOutgo>,
    MS: Stream<MasterIncome, MasterOutgo>,
> {
    pub auth_stream: AS,
    pub master_stream: MS,
    pub judge_service: Arc<judge::Service>,
    pub cert: Arc<Cert>,
    pub file_provider: P,
}

impl<
    P: file::providers::Provider + Sync + Send + 'static,
    AS: Stream<AuthIncome, AuthOutgo> + Send + Sync + 'static,
    MS: Stream<MasterIncome, MasterOutgo> + Send + Sync + 'static,
> App<P, AS, MS>
{
    pub fn start_judgment(
        self: &Arc<Self>,
        lang: Lang,
        solution: Box<[u8]>,
        package: Box<[u8]>,
    ) -> JoinHandle<crate::Result<judge::api::submission::Result>> {
        use server::stream::MasterOutgo as Outgo;
        let self_clone = Arc::clone(self);
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
                .context("archiving test verdict")
                .unwrap_or_else(|e| {
                    log::error!("{e}");
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
                    .context("sending test verdict")
                    .unwrap_or_else(|e| log::error!("{e}"));
            }
        });
        let self_clone = Arc::clone(self);

        tokio::spawn(async move {
            let package = archive::Archive::new(&*package);
            let result = Arc::clone(&self_clone.judge_service)
                .judge(package, lang, solution, sender)
                .await
                .context("judging");
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
                    .context("sending full verdict")
                    .unwrap_or_else(|e| {
                        log::error!("sending message error: {e}");
                    }),
                Err(e) => {
                    log::error!("{e}");
                    self_clone
                        .master_stream
                        .send(Outgo::Error {
                            msg: e.to_string().into(),
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
        let solution = challenge
            .solve(&self.cert, &policy::StandardPolicy::new())
            .context("solving auth challenge")?;
        self.auth_stream
            .send(Outgo::ChallengeSolution(solution))
            .await
    }

    async fn listen_master_stream(self: Arc<Self>) -> Result<()> {
        use server::stream::MasterIncome as Income;
        log::info!("master stream listener open");
        loop {
            let msg = self
                .master_stream
                .recv()
                .await
                .context("reading master message")?;
            log::trace!("master stream receive message {msg:#?}");
            match msg {
                Income::Run {
                    package_id,
                    lang,
                    data,
                } => {
                    let package = self
                        .file_provider
                        .get(package_id)
                        .await
                        .context("file provider get package")?;
                    _ = self.start_judgment(lang, data, package)
                }
                Income::Stop => self
                    .judge_service
                    .cancel_all_tests()
                    .await
                    .context("all tests cancelling")?,
                Income::Close => break,
            }
        }
        log::info!("master stream listner close");
        Result::<()>::Ok(())
    }

    async fn listen_auth_stream(self: Arc<Self>) -> Result<()> {
        use server::stream::AuthIncome as Income;
        log::info!("auth stream listener open");
        loop {
            let msg = self
                .auth_stream
                .recv()
                .await
                .context("reading auth message")?;
            log::trace!("auth stream receive message {msg:#?}");
            match msg {
                Income::Verdict(verdict) => {
                    if !verdict {
                        bail!("auth FAILED");
                    }
                }
                Income::Challenge(challenge) => self
                    .solve_challenge(challenge)
                    .await
                    .context("solving auth challenge")?,
            }
        }
    }

    pub async fn run(self: &Arc<Self>) -> Result<()> {
        tokio::select! {
            res = tokio::spawn(Arc::clone(self).listen_master_stream()) => res.context("listening master stream")?,
            res = tokio::spawn(Arc::clone(self).listen_auth_stream()) => res.context("listening auth stream")?,
        }
    }
}
