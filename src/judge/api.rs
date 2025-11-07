use super::Lang;

pub mod test {
    use std::sync::Arc;

    use serde::{Deserialize, Serialize};

    use crate::sandbox;
    #[derive(Debug, Clone)]
    pub struct Result {
        pub verdict: Verdict,
        pub time: f64,
        pub memory: u64,

        pub output: Arc<str>,
        pub message: Arc<str>,
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

    use super::Lang;

    #[derive(Debug, Deserialize, Clone)]
    #[serde(rename_all = "snake_case")]
    pub enum Type {
        Standard,
        Interactive,
    }

    #[derive(Debug, Deserialize, Clone, Copy)]
    pub struct Limits {
        pub time: f64,
        pub real_time: f64,

        pub memory: u64,
        pub stack: Option<usize>,
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
    pub struct Problem {
        pub r#type: Type,
        pub lang: Lang,
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
