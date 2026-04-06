use crate::prelude::*;

use serde::{Deserialize, Serialize};

use super::consts::*;

use super::sandbox::Command;

#[derive(Debug, Serialize, Deserialize, Clone, Copy, Hash, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    #[serde(rename = "g++")]
    Gpp,
    #[serde(rename = "python3")]
    Python,
}

impl TryFrom<&str> for Lang {
    type Error = Error;
    fn try_from(s: &str) -> Result<Self> {
        match &*s.to_lowercase() {
            "g++" => Ok(Lang::Gpp),
            "python3" => Ok(Lang::Python),
            _ => bail!("unknown language: {}", s),
        }
    }
}

impl Lang {
    pub fn command_to_run(&self, name: &str) -> Command {
        match self {
            Self::Gpp => Command::new(format!("./{name}")),
            Self::Python => {
                let mut cmd = Command::new(PYTHON3_BIN_PATH);
                cmd.arg(name);
                cmd
            }
        }
    }
}
pub mod test {
    use serde::{Deserialize, Serialize};

    use std::{fmt::Debug, sync::Arc};

    use crate::logger::short_str;

    use super::super::sandbox;
    #[derive(Clone)]
    pub struct Result {
        pub verdict: Verdict,
        pub time: f64,
        pub memory: u64,

        pub output: Arc<str>,
        pub message: Arc<str>,
    }

    impl Debug for Result {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("Result")
                .field("verdict", &self.verdict)
                .field("time", &self.time)
                .field("memory", &self.memory)
                .field("output", &short_str(&self.output))
                .field("message", &short_str(&self.message))
                .finish()
        }
    }

    #[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
    pub enum Verdict {
        Ok, //ok
        Wa, //wrong answer
        Pe, //presentation error
        Ml, //memory limit
        Tl, //time limit
        Re, //runtime error
        Ce, //compile error
        Te, //testing system error
        Sl, //stack limit
    }

    impl Verdict {
        pub fn from_run_status(status: sandbox::RunStatus) -> Option<Self> {
            Some(match status {
                sandbox::RunStatus::Ok => return None,
                sandbox::RunStatus::Tl => Self::Tl,
                sandbox::RunStatus::Ml => Self::Ml,
                sandbox::RunStatus::Re(_) => Self::Re,
                sandbox::RunStatus::Sg(_) => Self::Re,
            })
        }

        pub fn is_success(&self) -> bool {
            *self == Verdict::Ok
        }
    }

    impl std::fmt::Display for Verdict {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                f,
                "{}",
                match self {
                    Verdict::Ok => "OK",
                    Verdict::Wa => "WA",
                    Verdict::Pe => "PE",
                    Verdict::Ml => "ML",
                    Verdict::Tl => "TL",
                    Verdict::Re => "RE",
                    Verdict::Ce => "CE",
                    Verdict::Te => "TE",
                    Verdict::Sl => "SL",
                }
            )
        }
    }
}
pub mod submission {
    use serde::Deserialize;

    #[derive(Debug, Deserialize, Clone)]
    #[serde(rename_all = "snake_case")]
    pub enum Type {
        Standard,
        Interactive,
    }

    pub use bytesize::ByteSize;
    pub use tokio::time::Duration;

    use super::super::serde_with::de;

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

    #[derive(Debug, Clone)]
    pub enum Result {
        Ok {
            score: usize,
            groups_score: Box<[usize]>,
        },
        Ce(Box<str>),
        Te(Box<str>),
    }
}
