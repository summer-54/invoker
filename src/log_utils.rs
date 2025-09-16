use std::{fmt::Display, sync::Arc};
pub struct State(Option<(Arc<State>, Box<str>, Box<str>)>);
impl State {
    pub fn new() -> Arc<Self> {
        Arc::new(State(None))
    }
    pub fn push(self: &Arc<Self>, key: &str, value: &str) -> Arc<Self> {
        Arc::new(State(Some((
            Arc::clone(&self),
            Box::from(key),
            Box::from(value),
        ))))
    }
}

impl Display for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some((prev, key, value)) = &self.0 {
            write!(f, "{prev} {key}<{value}>")
        } else {
            Ok(())
        }
    }
}
