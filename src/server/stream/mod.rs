pub mod auth;
pub mod load;
pub mod master;
use super::{MappedRawMessage, RawMessage};
use crate::{prelude::*, server::MultiplexChannel};
use futures::stream::StreamExt;
use std::{marker::PhantomData, pin::Pin, sync::Arc};
use tokio::sync::Mutex;

pub type MasterIncome = master::Income;
pub type MasterOutgo = master::Outgo;
pub type MasterStream<C> = Stream<MasterIncome, MasterOutgo, C>;

pub type AuthIncome = auth::Income;
pub type AuthOutgo = auth::Outgo;
pub type AuthStream<C> = Stream<AuthIncome, AuthOutgo, C>;

pub type LoadIncome = load::Income;
pub type LoadOutgo = load::Outgo;
pub type LoadStream<C> = Stream<LoadIncome, LoadOutgo, C>;

pub trait Income: Sized {
    fn from_raw(msg: MappedRawMessage) -> Result<Self>;
}
pub trait Outgo: std::fmt::Debug {
    fn into_raw(self) -> RawMessage;
}

pub type SendResult = Pin<Box<dyn Future<Output = Result<()>> + Send + Sync>>;
pub type SendClosure<O, C> = Box<dyn Send + Sync + Fn(O, Arc<C>) -> SendResult>;

pub struct Stream<I: Income, O: Outgo, C: MultiplexChannel + ?Sized> {
    _pd: PhantomData<I>,
    receiver: Mutex<C::Receiver>,
    channel: Arc<C>,
    send_closure: SendClosure<O, C>,
}

impl<I: Income, O: Outgo, C: MultiplexChannel> Stream<I, O, C> {
    pub fn new(receiver: C::Receiver, channel: Arc<C>, send_closure: SendClosure<O, C>) -> Self {
        Self {
            _pd: Default::default(),
            receiver: Mutex::new(receiver),
            channel,
            send_closure,
        }
    }

    pub async fn recv(&self) -> Result<I> {
        let Some(raw) = self.receiver.lock().await.next().await else {
            bail!("reciever was closed");
        };
        I::from_raw(raw.into_mapped())
    }

    pub async fn send(&self, msg: O) -> Result<()> {
        (self.send_closure)(msg, Arc::clone(&self.channel)).await
    }
}
