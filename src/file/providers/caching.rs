use crate::prelude::*;

use bytesize::ByteSize;
use serde::{Deserialize, Serialize};
use tokio::{fs::File, io::AsyncWriteExt, sync::Mutex};

use lru::LruCache;

use crate::{
    logger::LogState,
    serde_with::{de, ser},
};

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    cache_dir: Box<Path>,
    #[serde(
        serialize_with = "ser::mb_lim_bytesize_to_mb_lim_kib",
        deserialize_with = "de::mb_lim_bytesize_from_mb_lim_kib"
    )]
    size_limit: MaybeLimited<ByteSize>,
}

impl std::default::Default for Config {
    fn default() -> Self {
        Self {
            cache_dir: Path::new("cache").into(),
            size_limit: Limited(ByteSize::kib(0)),
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
    lru: Mutex<(LruCache<Id, ByteSize>, ByteSize)>,
}

impl<P: Provider> CachingProvider<P> {
    pub async fn init(config_dir: Box<Path>, inner: P) -> Result<Self> {
        let config = <Config as configo::Config>::load(&config_dir)
            .await
            .context("loading config")?;
        if !config.cache_dir.as_ref().is_dir() {
            tokio::fs::create_dir(&config.cache_dir)
                .await
                .context("creating directory")?
        }

        let mut lru = LruCache::unbounded();
        let mut cur_size = ByteSize::b(0);

        let mut entries = tokio::fs::read_dir(&config.cache_dir)
            .await
            .context("reading directory")?;
        while let Some(entry) = entries.next_entry().await? {
            if entry.metadata().await?.is_dir() {
                continue;
            }

            let path = entry.path();
            let size = ByteSize::b(
                tokio::fs::File::open(&path)
                    .await
                    .context("opening cached file")?
                    .metadata()
                    .await
                    .context("parsing cached file's metadata")?
                    .len(),
            );

            cur_size += size;
            let id = path
                .file_name()
                .ok_or(anyhow!("SOoooo strange error, write issue about it"))?;

            lru.push(id.display().to_string().parse::<Id>()?, size);
        }

        if let Limited(lim) = config.size_limit
            && lim < cur_size
        {
            let excess_size = cur_size - lim;
            Self::shrink(&config, &mut lru, &mut cur_size, excess_size)
                .await
                .context("shrinking cache on init")?;
        }
        let this = Self {
            inner,
            config,
            lru: Mutex::new((lru, cur_size)),
        };

        Ok(this)
    }
    fn path_by_id(&self, id: Id) -> Box<Path> {
        self.config
            .cache_dir
            .join(Path::new(&format!("{id}")))
            .into()
    }

    async fn shrink(
        config: &Config,
        lru: &mut LruCache<Id, ByteSize>,
        cur_size: &mut ByteSize,
        mut excess_size: ByteSize,
    ) -> Result<()> {
        log::trace!("{cur_size} {excess_size}");
        while excess_size > ByteSize::b(0) {
            let (popped_id, size) = lru.pop_lru().context("something strange, issue pls")?;

            let path = config.cache_dir.join(popped_id.to_string());

            tokio::fs::remove_file(path)
                .await
                .context("removing file that overflow cache")?;
            let log_state = LogState::new()
                .push("package_id", popped_id)
                .push("size(mib)", size.as_mib());

            *cur_size -= size;
            excess_size -= std::cmp::min(size, excess_size);
            log::trace!("{log_state} removed from cache");
        }
        Ok(())
    }
}

impl<P: Provider + Sync + Send> Provider for CachingProvider<P> {
    async fn get(&self, id: Id) -> Result<Box<[u8]>> {
        let path = self.path_by_id(id);
        let (lru, cur_size) = &mut *self.lru.lock().await;
        if let Some(&size) = lru.get(&id) {
            let log_state = LogState::new()
                .push("package_id", id)
                .push("size(mib)", size.as_mib());

            match tokio::fs::read(&path).await {
                Ok(data) if data.len() as u64 == size.as_u64() => {
                    return Ok(data.into_boxed_slice());
                }
                _ => {
                    log::warn!("{log_state} cache file missing/corrupted. Evicting metadata.");
                    if let Some(size) = lru.pop(&id) {
                        *cur_size -= size;
                    }
                }
            }
        }

        let data = self
            .inner
            .get(id)
            .await
            .context("getting inner provilder")?;
        let size = ByteSize::b(data.len() as u64);
        let log_state = LogState::new()
            .push("package_id", id)
            .push("size(mib)", size.as_mib());

        if let Limited(limit) = self.config.size_limit {
            if limit < size {
                log::trace!("{log_state} is too big to be in the cache");
                return Ok(data);
            } else if *cur_size + size > limit {
                Self::shrink(&self.config, lru, cur_size, *cur_size + size - limit).await?;
            }
        }

        File::create(&path)
            .await?
            .write_all(&data)
            .await
            .context("creating file")?;

        lru.push(id, size);
        *cur_size += size;

        log::trace!("{log_state} added into cache");
        Ok(data)
    }
}
