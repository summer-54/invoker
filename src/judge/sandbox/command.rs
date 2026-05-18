use crate::prelude::*;

use bytesize::ByteSize;
use tokio::time::Duration;

use crate::types::MaybeLimited;

#[derive(Debug, Clone)]
pub struct Command {
    pub(super) program: Box<str>,
    pub(super) args: Vec<Box<str>>,

    pub(super) time_limit: Option<MaybeLimited<Duration>>, // Time limit (in seconds)
    pub(super) memory_limit: Option<MaybeLimited<ByteSize>>, // Memory limit (in KiB)
    pub(super) real_time_limit: Option<MaybeLimited<Duration>>, // Real time limit (in seconds)
    pub(super) extra_time_limit: Option<Duration>,         // Extra time limit (in seconds)
    pub(super) stack_limit: Option<MaybeLimited<ByteSize>>, // Stack limit (in KiB)
    pub(super) count_files_limit: Option<MaybeLimited<usize>>,
    pub(super) count_process_limit: Option<MaybeLimited<usize>>,
    pub(super) use_env: bool,

    pub(super) open_dirs: Vec<Box<str>>,

    pub(super) stdin: Option<Box<Path>>,
    pub(super) stdout: Option<Box<Path>>,
    pub(super) stderr: Option<Box<Path>>,
}

#[allow(dead_code)]
impl Command {
    pub fn new(program: impl AsRef<str>) -> Self {
        Self {
            program: Box::from(program.as_ref()),
            args: vec![],

            time_limit: Default::default(),
            memory_limit: Default::default(),
            real_time_limit: Default::default(),
            extra_time_limit: Default::default(),
            stack_limit: Default::default(),
            count_files_limit: Default::default(),
            count_process_limit: Default::default(),
            open_dirs: vec![],
            use_env: false,

            stdin: Default::default(),
            stdout: Default::default(),
            stderr: Default::default(),
        }
    }

    pub fn arg(&mut self, arg: impl AsRef<str>) -> &mut Self {
        self.args.push(Box::from(arg.as_ref()));
        self
    }

    pub fn args(&mut self, args: impl IntoIterator<Item = impl AsRef<str>>) -> &mut Self {
        for arg in args {
            self.arg(arg);
        }
        self
    }

    pub fn stdin(&mut self, path: impl AsRef<Path>) -> &mut Self {
        self.stdin = Some(Box::from(path.as_ref()));
        self
    }

    pub fn stdout(&mut self, path: impl AsRef<Path>) -> &mut Self {
        self.stdout = Some(Box::from(path.as_ref()));
        self
    }

    pub fn stderr(&mut self, path: impl AsRef<Path>) -> &mut Self {
        self.stdout = Some(Box::from(path.as_ref()));
        self
    }

    pub fn time(&mut self, cfg: MaybeLimited<Duration>) -> &mut Self {
        self.time_limit = Some(cfg);
        self
    }
    pub fn real_time(&mut self, cfg: MaybeLimited<Duration>) -> &mut Self {
        self.real_time_limit = Some(cfg);
        self
    }
    pub fn extra_time(&mut self, cfg: Duration) -> &mut Self {
        self.extra_time_limit = Some(cfg);
        self
    }

    pub fn memory(&mut self, cfg: MaybeLimited<ByteSize>) -> &mut Self {
        self.memory_limit = Some(cfg);
        self
    }
    pub fn stack(&mut self, cfg: MaybeLimited<ByteSize>) -> &mut Self {
        self.stack_limit = Some(cfg);
        self
    }

    pub fn count_files(&mut self, cfg: MaybeLimited<usize>) -> &mut Self {
        self.count_files_limit = Some(cfg);
        self
    }
    pub fn count_process(&mut self, cfg: MaybeLimited<usize>) -> &mut Self {
        self.count_process_limit = Some(cfg);
        self
    }

    pub fn use_env(&mut self) -> &mut Self {
        self.use_env = true;
        self
    }

    pub fn open_dir(&mut self, path: impl AsRef<str>) -> &mut Self {
        self.args.push(Box::from(path.as_ref()));
        self
    }
    pub fn open_dirs(&mut self, paths: impl IntoIterator<Item = impl AsRef<str>>) -> &mut Self {
        for path in paths {
            self.arg(path);
        }
        self
    }
}
