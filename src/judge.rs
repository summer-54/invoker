use crate::App;
use crate::Result;
use std::sync::Arc;

pub async fn judge(app: Arc<App>, packege_path: &str) -> Result<()> {
    let (test_count) = todo!("parse package");

    for i in 0..test_count {
        let sandbox = app.isolate.init_box().await?;
    }

    Ok(())
}

pub fn stop_all(app: Arc<App>) -> Result<()> {
    todo!()
}
