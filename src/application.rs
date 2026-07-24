use crate::prelude::*;

use toaster_lib_rs::auth::{Cert, Challenge, policy};

use std::sync::Arc;

use crate::{
    Result, judge,
    server::{
        self,
        stream::{
            AuthIncome, AuthOutgo, JudgeIncome, JudgeOutgo, MasterIncome, MasterOutgo, Stream,
        },
    },
};

pub struct App<
    AS: Stream<AuthIncome, AuthOutgo>,
    MS: Stream<MasterIncome, MasterOutgo>,
    JS: Stream<JudgeIncome, JudgeOutgo>,
> {
    pub auth_stream: AS,
    pub master_stream: MS,
    pub judge_service: Arc<judge::Service<JS>>,
    pub cert: Arc<Cert>,
}

impl<
    AS: Stream<AuthIncome, AuthOutgo> + Send + Sync + 'static,
    MS: Stream<MasterIncome, MasterOutgo> + Send + Sync + 'static,
    JS: Stream<JudgeIncome, JudgeOutgo> + Send + Sync + 'static,
> App<AS, MS, JS>
{
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
            let msg = match self
                .master_stream
                .recv()
                .await
                .context("reading master message")
            {
                Ok(msg) => msg,
                Err(e) => {
                    log::error!("{e}");
                    continue;
                }
            };
            log::trace!("master stream receive message {msg:#?}");
            match msg {
                Income::Close => break,
            }
        }
        log::info!("master stream listener close");
        Result::<()>::Ok(())
    }

    async fn listen_auth_stream(self: Arc<Self>) -> Result<()> {
        use server::stream::AuthIncome as Income;
        log::info!("auth stream listener open");
        loop {
            let msg = match self
                .auth_stream
                .recv()
                .await
                .context("reading auth message")
            {
                Ok(msg) => msg,
                Err(e) => {
                    log::error!("{e}");
                    continue;
                }
            };
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
        let this = Arc::clone(self);
        tokio::select! {
            res = tokio::spawn(async move {this.judge_service.run().await}) => res.context("listening master stream")?,
            res = tokio::spawn(Arc::clone(self).listen_master_stream()) => res.context("listening master stream")?,
            res = tokio::spawn(Arc::clone(self).listen_auth_stream()) => res.context("listening auth stream")?,
        }
    }
}
