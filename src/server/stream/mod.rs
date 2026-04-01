pub mod auth;
pub mod master;
use super::{MappedRawMessage, MultiplexChannel, RawMessage};
use crate::prelude::*;
use std::{marker::PhantomData, sync::Weak};
use tokio::sync::{Mutex, mpsc::UnboundedReceiver};

pub type MasterIncome = master::Income;
pub type MasterOutgo = master::Outgo;
pub type MasterStream<C> = Stream<MasterIncome, MasterOutgo, C>;

pub type AuthIncome = auth::Income;
pub type AuthOutgo = auth::Outgo;
pub type AuthStream<C> = Stream<AuthIncome, AuthOutgo, C>;

pub trait Income: Sized {
    fn from_raw(msg: MappedRawMessage) -> Result<Self>;
}
pub trait Outgo {
    fn into_raw(self) -> RawMessage;
}

pub struct Stream<I: Income, O: Outgo, C: MultiplexChannel> {
    _pd: PhantomData<(I, O)>,
    name: Box<str>,
    receiver: Mutex<UnboundedReceiver<RawMessage>>,
    channel: Weak<C>,
}

impl<I: Income, O: Outgo, C: MultiplexChannel> Stream<I, O, C> {
    pub fn new(name: &str, receiver: UnboundedReceiver<RawMessage>, channel: Weak<C>) -> Self {
        Self {
            _pd: Default::default(),
            name: Box::from(name),
            receiver: Mutex::new(receiver),
            channel,
        }
    }
    pub async fn send(&self, msg: O) -> Result<()> {
        let Some(channel) = self.channel.upgrade() else {
            bail!("channel doesn't exists");
        };
        channel.send(&self.name, msg.into_raw()).await
    }
    pub async fn recv(&self) -> Result<I> {
        let Some(raw) = self.receiver.lock().await.recv().await else {
            bail!("reciever was closed");
        };
        I::from_raw(raw.into_mapped())
    }
}
