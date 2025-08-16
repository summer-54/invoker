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
            groups_score: Box<[usize]>,
        },
        TestVerdict {
            test_id: usize,
            verdict: Verdict,
            time: f64,
            memory: usize,
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
}
