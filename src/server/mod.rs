#[cfg(not(feature = "mock"))]
pub mod websocket;

use crate::Result;

const VISIBLE_DATA_LEN: usize = 5;

#[allow(dead_code)]
pub mod income {
    use std::future;

    use super::{Result, VISIBLE_DATA_LEN};

    pub enum Msg {
        Start { data: Box<[u8]> },
        Stop,
        Close,
    }

    impl std::fmt::Debug for Msg {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::Start { data } => f
                    .debug_struct("Start")
                    .field("data", &Box::<[u8]>::from(&data[..VISIBLE_DATA_LEN]))
                    .finish(),
                Self::Stop => write!(f, "Stop"),
                Self::Close => write!(f, "Close"),
            }
        }
    }

    pub trait Receiver: Send + Sync {
        fn recv(&self) -> impl Future<Output = Result<Msg>> + Send;
    }

    pub struct MockReceiver;
    impl Receiver for MockReceiver {
        fn recv(&self) -> impl Future<Output = Result<Msg>> + Send {
            future::pending()
        }
    }
}
#[allow(dead_code)]
pub mod outgo {
    use colored::Colorize;

    use super::{Result, VISIBLE_DATA_LEN};
    use crate::judge::api::test::Verdict;

    #[derive(Debug)]
    pub enum FullVerdict {
        Ok {
            score: usize,
            groups_score: Box<[usize]>,
        },
        Ce(Box<str>),
        Te(Box<str>),
    }

    // #[derive(Debug)]
    pub enum Msg {
        Token(uuid::Uuid),
        FullVerdict(FullVerdict),
        TestVerdict {
            test_id: usize,
            verdict: Verdict,
            time: f64,
            memory: u64,
            data: Box<[u8]>,
        },
        Exited {
            code: u8,
            data: Box<str>,
        },
        Error {
            msg: Box<str>,
        },
        OpError {
            msg: Box<str>,
        },
    }

    impl std::fmt::Debug for Msg {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::Token(arg0) => f.debug_tuple("Token").field(arg0).finish(),
                Self::FullVerdict(arg0) => f.debug_tuple("FullVerdict").field(arg0).finish(),
                Self::TestVerdict {
                    test_id,
                    verdict,
                    time,
                    memory,
                    data,
                } => f
                    .debug_struct("TestVerdict")
                    .field("test_id", test_id)
                    .field("verdict", verdict)
                    .field("time", time)
                    .field("memory", memory)
                    .field("data", &Box::<[u8]>::from(&data[..VISIBLE_DATA_LEN]))
                    .finish(),
                Self::Exited { code, data } => f
                    .debug_struct("Exited")
                    .field("code", code)
                    .field("data", data)
                    .finish(),
                Self::Error { msg } => f.debug_struct("Error").field("msg", msg).finish(),
                Self::OpError { msg } => f.debug_struct("OpError").field("msg", msg).finish(),
            }
        }
    }

    pub trait Sender: Send + Sync {
        fn send(&self, msg: Msg) -> impl Future<Output = Result<()>> + Send;
    }

    pub struct MockSender;
    impl Sender for MockSender {
        fn send(&self, msg: Msg) -> impl Future<Output = Result<()>> + Send {
            log::info!(
                "{}\n {:?}",
                "---+++==< MESSAGE SENDED >==+++---".bright_magenta(),
                msg
            );
            futures::future::ready(Ok(()))
        }
    }
}
