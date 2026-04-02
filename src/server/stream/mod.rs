pub mod auth;
pub mod master;
use super::{MappedRawMessage, RawMessage};
use crate::prelude::*;
use std::{marker::PhantomData, pin::Pin};
use tokio::sync::{Mutex, mpsc::UnboundedReceiver};

pub type MasterIncome = master::Income;
pub type MasterOutgo = master::Outgo;
pub type MasterStream = Stream<MasterIncome, MasterOutgo>;

pub type AuthIncome = auth::Income;
pub type AuthOutgo = auth::Outgo;
pub type AuthStream = Stream<AuthIncome, AuthOutgo>;

pub trait Income: Sized + std::fmt::Debug {
    fn from_raw(msg: MappedRawMessage) -> Result<Self>;
}
pub trait Outgo: std::fmt::Debug {
    fn into_raw(self) -> RawMessage;
}

pub type SendResult = Pin<Box<dyn Future<Output = Result<()>> + Send + Sync>>;
pub type SendClosure<O> = Box<dyn Send + Sync + Fn(O) -> SendResult>;

pub struct Stream<I: Income, O: Outgo> {
    _pd: PhantomData<(I, O)>,
    receiver: Mutex<UnboundedReceiver<RawMessage>>,
    send_closure: SendClosure<O>,
}

impl<I: Income, O: Outgo> Stream<I, O> {
    pub fn new(receiver: UnboundedReceiver<RawMessage>, send_closure: SendClosure<O>) -> Self {
        Self {
            _pd: Default::default(),
            receiver: Mutex::new(receiver),
            send_closure,
        }
    }

    pub async fn recv(&self) -> Result<I> {
        let Some(raw) = self.receiver.lock().await.recv().await else {
            bail!("reciever was closed");
        };
        I::from_raw(raw.into_mapped())
    }

    pub async fn send(&self, msg: O) -> Result<()> {
        (self.send_closure)(msg).await
    }
}
