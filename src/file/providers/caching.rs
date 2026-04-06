use crate::prelude::*;

use serde::{Deserialize, Serialize};
use tokio::{fs::File, io::AsyncWriteExt, sync::RwLock};

use std::collections::HashSet;

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    cache_dir: Box<Path>,
}

impl std::default::Default for Config {
    fn default() -> Self {
        Self {
            cache_dir: Path::new("cache").into(),
        }
    }
}

impl configo::Config for Config {
    const NAME: &'static str = "cache";
}

use super::{super::Id, Provider};
pub struct CachingProvider<P: Provider> {
    inner: P,
    config: Config,
    marks: RwLock<HashSet<Id>>,
}

impl<P: Provider> CachingProvider<P> {
    pub async fn init(config_dir: Box<Path>, inner: P) -> Result<Self> {
        let config = <Config as configo::Config>::load(&config_dir).await?;
        if !config.cache_dir.as_ref().is_dir() {
            tokio::fs::create_dir(&config.cache_dir)
                .await
                .context("creating directory")?
        }

        let mut marks = HashSet::new();

        let mut entries = tokio::fs::read_dir(&config.cache_dir)
            .await
            .context("reading directory")?;
        while let Some(entry) = entries.next_entry().await? {
            if entry.metadata().await?.is_dir() {
                continue;
            }

            let path = entry.path();
            let id = path
                .file_name()
                .ok_or(anyhow!("SOoooo strange error, write issue about it"))?;
            marks.insert(id.display().to_string().parse::<Id>()?);
        }

        Ok(Self {
            inner,
            config,
            marks: RwLock::new(marks),
        })
    }
    fn path_by_id(&self, id: Id) -> Box<Path> {
        self.config
            .cache_dir
            .join(Path::new(&format!("{id}")))
            .into()
    }
}

impl<P: Provider + Sync + Send> Provider for CachingProvider<P> {
    async fn get(&self, id: Id) -> Result<Box<[u8]>> {
        let path = self.path_by_id(id);
        if self.marks.read().await.contains(&id) {
            return Ok(tokio::fs::read(path)
                .await
                .context("reading file")?
                .into_boxed_slice());
        }

        let data = self
            .inner
            .get(id)
            .await
            .context("getting inner provilder")?;
        File::create(&path)
            .await?
            .write_all(&data)
            .await
            .context("creating file")?;
        self.marks.write().await.insert(id);

        Ok(data)
    }
}
