use crate::prelude::*;

use super::Id;

use crate::server::stream::{LoadIncome, LoadOutgo, Stream};

use tokio::sync::Mutex;

pub struct StreamProvider<S: Stream<LoadIncome, LoadOutgo>> {
    stream: Mutex<S>,
}

impl<S: Stream<LoadIncome, LoadOutgo>> StreamProvider<S> {
    pub fn new(stream: S) -> Self {
        Self {
            stream: Mutex::new(stream),
        }
    }
}

impl<S: Stream<LoadIncome, LoadOutgo> + Send> super::Provider for StreamProvider<S> {
    async fn get(&self, id: Id) -> Result<Box<[u8]>> {
        let stream = self.stream.lock().await;
        stream.send(LoadOutgo::Load { package_id: id }).await?;
        match stream.recv().await? {
            LoadIncome::Package(data) => Ok(data),
        }
    }
}
