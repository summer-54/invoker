use crate::prelude::*;
use std::{fmt::Display, sync::Arc};

pub struct LogState(Option<(Arc<LogState>, Box<str>, Box<str>)>);
impl LogState {
    pub fn new() -> Arc<Self> {
        Arc::new(LogState(None))
    }
    pub fn push(self: &Arc<Self>, key: &str, value: &str) -> Arc<Self> {
        Arc::new(LogState(Some((
            Arc::clone(&self),
            Box::from(key),
            Box::from(value),
        ))))
    }
}

impl Display for LogState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some((prev, key, value)) = &self.0 {
            if prev.0.is_some() {
                write!(f, "{prev} ")?;
            }
            write!(f, "{}<{}>", key.green(), value.cyan())
        } else {
            Ok(())
        }
    }
}
