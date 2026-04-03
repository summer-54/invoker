use crate::prelude::*;

use super::Id;

use crate::server::{
    MultiplexChannel,
    stream::{LoadIncome, LoadOutgo, LoadStream},
};

use std::sync::Arc;
use tokio::sync::Mutex;

pub struct StreamProvider<C: MultiplexChannel> {
    stream: Mutex<LoadStream<C>>,
}

impl<C: MultiplexChannel> StreamProvider<C> {
    pub async fn new(channel: Arc<C>, stream_name: &str) -> Self {
        Self {
            stream: Mutex::new(channel.new_stream(stream_name).await),
        }
    }
}

impl<C: MultiplexChannel + Sync + Send> super::Provider for StreamProvider<C> {
    async fn get(&self, id: Id) -> Result<Box<[u8]>> {
        let stream = self.stream.lock().await;
        stream.send(LoadOutgo::Load { package_id: id }).await?;
        match stream.recv().await? {
            LoadIncome::Package(data) => Ok(data),
        }
    }
}
