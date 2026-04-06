use crate::prelude::*;

use std::{fmt::Display, sync::Arc};

type LogStateInner = Option<(
    Arc<LogState>,
    Box<dyn Display + Sync + Send>,
    Box<dyn Display + Sync + Send>,
)>;
pub struct LogState(LogStateInner);
impl LogState {
    pub fn new() -> Arc<Self> {
        Arc::new(LogState(None))
    }
    pub fn push(
        self: &Arc<Self>,
        key: impl Display + 'static + Sync + Send,
        value: impl Display + 'static + Sync + Send,
    ) -> Arc<Self> {
        Arc::new(LogState(Some((
            Arc::clone(self),
            Box::from(key),
            Box::from(value),
        ))))
    }
}

impl LogState {
    pub fn inner_fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some((prev, key, value)) = &self.0 {
            if prev.0.is_some() {
                write!(f, "{prev} ")?;
            }
            write!(
                f,
                "{} = {}, ",
                format!("{key}").green(),
                format!("{value}").cyan()
            )
        } else {
            Ok(())
        }
    }
}

impl Display for LogState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[")?;
        self.inner_fmt(f)?;
        write!(f, "]")
    }
}

const VISIBLE_DATA_LEN: usize = 30;

pub fn short_slice<T>(data: &[T]) -> &[T] {
    &data[..std::cmp::min(data.len(), VISIBLE_DATA_LEN)]
}

pub fn short_str(data: &str) -> &str {
    let len = data
        .char_indices()
        .nth(VISIBLE_DATA_LEN)
        .map(|(i, _)| i)
        .unwrap_or(data.len());

    &data[..std::cmp::min(data.len(), len)]
}
