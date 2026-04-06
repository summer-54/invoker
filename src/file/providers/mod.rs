pub mod caching;
pub mod stream;

use crate::prelude::*;

use super::Id;

pub trait Provider {
    fn get(&self, id: Id) -> impl Future<Output = Result<Box<[u8]>>> + Send;
}
