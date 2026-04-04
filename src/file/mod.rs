pub mod stream_provider;

use crate::prelude::*;

use tokio::{fs::File, io::AsyncWriteExt, sync::RwLock};

use std::{collections::HashSet, path::Path, str::FromStr};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Id(u128);

impl FromStr for Id {
    type Err = <u128 as FromStr>::Err;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        s.parse::<u128>().map(Self)
    }
}

impl std::fmt::Display for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

pub trait Provider {
    fn get(&self, id: Id) -> impl Future<Output = Result<Box<[u8]>>> + Send;
}

pub struct CachingProvider<P: Provider> {
    inner: P,
    dir: Box<Path>,
    marks: RwLock<HashSet<Id>>,
}

impl<P: Provider> CachingProvider<P> {
    pub async fn init(dir: impl AsRef<Path>, inner: P) -> Result<Self> {
        if !dir.as_ref().is_dir() {
            tokio::fs::create_dir(&dir).await?
        }

        let mut marks = HashSet::new();

        let mut entries = tokio::fs::read_dir(&dir).await?;
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
            dir: dir.as_ref().into(),
            marks: RwLock::new(marks),
        })
    }
    fn path_by_id(&self, id: Id) -> Box<Path> {
        self.dir.join(Path::new(&format!("{id}"))).into()
    }
}

impl<P: Provider + Sync + Send> Provider for CachingProvider<P> {
    async fn get(&self, id: Id) -> Result<Box<[u8]>> {
        let path = self.path_by_id(id);
        if self.marks.read().await.contains(&id) {
            return Ok(tokio::fs::read(path).await?.into_boxed_slice());
        }

        let data = self.inner.get(id).await?;
        File::create(&path).await?.write_all(&data).await?;
        self.marks.write().await.insert(id);

        Ok(data)
    }
}
