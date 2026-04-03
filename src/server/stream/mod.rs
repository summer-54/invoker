pub mod auth;
pub mod load;
pub mod master;
use super::{MappedRawMessage, RawMessage};
use crate::prelude::*;
use std::pin::Pin;

pub type MasterIncome = master::Income;
pub type MasterOutgo = master::Outgo;
pub type MasterStream = Stream<MasterIncome, MasterOutgo>;

pub type AuthIncome = auth::Income;
pub type AuthOutgo = auth::Outgo;
pub type AuthStream = Stream<AuthIncome, AuthOutgo>;

pub type LoadIncome = load::Income;
pub type LoadOutgo = load::Outgo;
pub type LoadStream = Stream<LoadIncome, LoadOutgo>;

pub trait Income: Sized {
    fn from_raw(msg: MappedRawMessage) -> Result<Self>;
}
pub trait Outgo: std::fmt::Debug {
    fn into_raw(self) -> RawMessage;
}

pub type SendClosureResult = Pin<Box<dyn Future<Output = Result<()>> + Send + Sync>>;
pub type SendClosure<O> = Box<dyn Send + Sync + Fn(O) -> SendClosureResult>;
pub type ReceiverClosureResult<I> = Pin<Box<dyn Future<Output = Result<I>> + Send + Sync>>;
pub type ReceiverClosure<I> = Box<dyn Send + Sync + Fn() -> ReceiverClosureResult<I>>;

pub struct Stream<I: Income, O: Outgo> {
    receive_closure: ReceiverClosure<I>,
    send_closure: SendClosure<O>,
}

impl<I: Income + 'static, O: Outgo> Stream<I, O> {
    pub(super) fn new(send_closure: SendClosure<O>, receive_closure: ReceiverClosure<I>) -> Self {
        Self {
            send_closure,
            receive_closure,
        }
    }

    pub fn mock() -> Self {
        Self {
            send_closure: Box::new(|_: O| Box::pin(async { Result::Ok(()) })),
            receive_closure: Box::new(|| Box::pin(std::future::pending())),
        }
    }

    pub async fn recv(&self) -> Result<I> {
        (self.receive_closure)().await
    }

    pub async fn send(&self, msg: O) -> Result<()> {
        (self.send_closure)(msg).await
    }
}
