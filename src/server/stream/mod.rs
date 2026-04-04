pub mod auth;
pub mod load;
pub mod master;
use super::{MappedRawMessage, RawMessage};
use crate::prelude::*;

pub type MasterIncome = master::Income;
pub type MasterOutgo = master::Outgo;

pub type AuthIncome = auth::Income;
pub type AuthOutgo = auth::Outgo;

pub type LoadIncome = load::Income;
pub type LoadOutgo = load::Outgo;

pub trait Income: Sized {
    fn from_raw(msg: MappedRawMessage) -> Result<Self>;
}
pub trait Outgo {
    fn into_raw(self) -> RawMessage;
}

impl<T: TryFrom<MappedRawMessage, Error = impl std::error::Error + Sync + Send + 'static>> Income
    for T
{
    fn from_raw(msg: MappedRawMessage) -> Result<Self> {
        Ok(Self::try_from(msg)?)
    }
}

impl<T: Into<RawMessage>> Outgo for T {
    fn into_raw(self) -> RawMessage {
        self.into()
    }
}

pub trait Stream<I: Income, O: Outgo> {
    fn recv(&self) -> impl Future<Output = Result<I>> + Send;
    fn send(&self, msg: O) -> impl Future<Output = Result<()>> + Send;
}
