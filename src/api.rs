#[derive(Debug)]
pub enum Verdict {}
impl ToString for Verdict {
    fn to_string(&self) -> String {
        todo!()
    }
}

pub mod income {
    #[derive(Debug)]
    pub enum Msg {
        Start { data: Box<[u8]> },
        Stop,
        Close,
    }
}

pub mod outgo {
    use super::Verdict;

    #[derive(Debug)]
    pub enum Msg {
        FullTaskVerdict {
            verdict: Verdict,
            data: Box<str>,
        },
        SubTaskVerdict {
            subtask_id: usize,
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
