use crate::prelude::*;

use super::Id;

use crate::server::stream::{LoadIncome, LoadOutgo, LoadStream};

use tokio::sync::Mutex;

pub struct StreamProvider {
    stream: Mutex<LoadStream>,
}

impl StreamProvider {
    pub fn new(stream: LoadStream) -> Self {
        Self {
            stream: Mutex::new(stream),
        }
    }
}

impl super::Provider for StreamProvider {
    async fn get(&self, id: Id) -> Result<Box<[u8]>> {
        let stream = self.stream.lock().await;
        stream.send(LoadOutgo::Load { package_id: id }).await?;
        match stream.recv().await? {
            LoadIncome::Package(data) => Ok(data),
        }
    }
}
