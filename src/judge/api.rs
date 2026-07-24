use super::consts::*;

use super::sandbox::Command;

pub use judge_lib::Lang;
use toaster_lib_rs::judge::{self as judge_lib};

pub fn command_to_run(lang: Lang, name: &str) -> Command {
    match lang {
        Lang::Gpp => Command::new(format!("./{name}")),
        Lang::Python => {
            let mut cmd = Command::new(PYTHON3_BIN_PATH);
            cmd.arg(name);
            cmd
        }
    }
}
pub mod test {
    use std::{fmt::Debug, sync::Arc};

    use anyhow::Context;
    use tar_archive_rs::ArchiveItem;

    pub use super::judge_lib::test::{Result, ResultPayload, Verdict};

    #[derive(Debug, Clone)]
    pub struct Artifact {
        pub result: Result,
        pub output: Arc<str>,
        pub message: Arc<str>,
    }

    impl Artifact {
        pub async fn into_payload(self, id: usize) -> crate::prelude::Result<ResultPayload> {
            Ok(ResultPayload {
                result: self.result,
                id,
                data: tar_archive_rs::pack(&[
                    ArchiveItem {
                        path: "output",
                        data: self.output.as_bytes(),
                    },
                    ArchiveItem {
                        path: "message",
                        data: self.message.as_bytes(),
                    },
                ])
                .await
                .context("archiving test verdict")?,
            })
        }
    }
}
pub mod submission {
    pub use super::judge_lib::submission::Result;
    use serde::Deserialize;

    #[derive(Debug, Deserialize, Clone)]
    #[serde(rename_all = "snake_case")]
    pub enum Type {
        Standard,
        Interactive,
    }

    pub use bytesize::ByteSize;
    pub use tokio::time::Duration;

    use crate::serde_with::de;

    #[derive(Debug, Deserialize, Clone, Copy)]
    pub struct Limits {
        #[serde(deserialize_with = "de::duration_from_secs")]
        pub time: Duration,
        #[serde(deserialize_with = "de::duration_from_secs")]
        pub real_time: Duration,

        #[serde(deserialize_with = "de::bytesize_from_kib")]
        pub memory: ByteSize,
        #[serde(deserialize_with = "de::option_bytesize_from_option_kib")]
        pub stack: Option<ByteSize>,
    }

    #[derive(Debug, Deserialize, Clone)]
    pub struct TestsRange(pub usize, pub usize);

    #[derive(Debug, Deserialize, Clone)]
    pub struct Group {
        pub id: usize,
        pub range: TestsRange,
        pub cost: usize,
        pub depends: Box<[usize]>,
    }

    #[derive(Debug, Deserialize)]
    pub struct Task {
        #[serde(rename = "type")]
        pub ty: Type,

        pub limits: Limits,
        pub groups: Box<[Group]>,
    }
}
