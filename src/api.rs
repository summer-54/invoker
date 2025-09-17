use crate::Result;
pub mod income {
    use super::Result;

    #[derive(Debug)]
    pub enum Msg {
        Start { data: Box<[u8]> },
        Stop,
        Close,
    }

    pub trait Receiver: Send + Sync {
        fn recv(&self) -> impl Future<Output = Result<Msg>> + Send;
    }
}

pub mod outgo {
    use super::Result;
    use crate::judge::Verdict;

    #[derive(Debug)]
    pub enum FullVerdict {
        Ok {
            score: usize,
            groups_score: Box<[usize]>,
        },
        Ce(Box<str>),
        Te(Box<str>),
    }

    #[derive(Debug)]
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

    pub trait Sender: Send + Sync {
        fn send(&self, msg: Msg) -> impl Future<Output = Result<()>> + Send;
    }
}
