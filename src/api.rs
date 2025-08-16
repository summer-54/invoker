pub mod income {
    #[derive(Debug)]
    pub enum Msg {
        Start { data: Box<[u8]> },
        Stop,
        Close,
    }
}

pub mod outgo {
    use crate::judge::Verdict;
    #[derive(Debug)]
    pub enum Msg {
        FullVerdict {
            score: usize,
            data: Box<str>,
        },
        TestVerdict {
            test_id: usize,
            verdict: Verdict,
            data: Box<str>,
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
}
