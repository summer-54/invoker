use crate::App;
use crate::Result;
use std::sync::Arc;

pub fn judge(app: Arc<App>, packege_path: &str) -> Result<()> {
    let (test_count) = todo!("parse package");

    for i in 0..test_count {}
}

pub fn stop_all(app: Arc<App>) -> Result<()> {
    todo!()
}
