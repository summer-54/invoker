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
}
