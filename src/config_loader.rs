use serde::{Deserialize, Serialize};

use crate::LogState;

pub trait Config: Default + Serialize + for<'de> Deserialize<'de> {
    const NAME: &'static str;
    async fn load(dir: &str) -> Self {
        let state = LogState::new();
        state.push("name", Self::NAME);

        let path = format!("{dir}/{}.yaml", Self::NAME).into_boxed_str();
        if !tokio::fs::try_exists(&*path).await.unwrap() {
            let this = Self::default();

            tokio::fs::write(&*path, serde_yml::to_string(&this).unwrap())
                .await
                .unwrap();

            log::warn!("{state} config not found by path: {path}");
            log::info!("{state} config was automaticly created by path: {path}");

            this
        } else {
            let this =
                serde_yml::from_str(&tokio::fs::read_to_string(&*path).await.unwrap()).unwrap();
            log::trace!("{state} config was loaded");
            this
        }
    }
}
