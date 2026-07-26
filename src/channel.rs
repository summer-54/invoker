use crate::prelude::*;

use crate::logger::LogState;

pub struct Channel(pub Box<Path>);

impl Channel {
    pub async fn new(dir: impl AsRef<Path>) -> Result<Channel> {
        let id: u64 = rand::random();
        let path = dir.as_ref().join(format!("{id}")).into_boxed_path();

        let status = tokio::process::Command::new("mkfifo")
            .arg("-m")
            .arg("666")
            .arg(&*path)
            .status()
            .await
            .context("executing command mkfifo")?;
        if status.success() {
            let log_state = LogState::new()
                .push("id", id.to_string())
                .push("path", path.as_ref().display().to_string());
            log::trace!(
                "{log_state} new channel created at {}",
                path.display().to_string().bright_white()
            );

            Ok(Channel(path))
        } else {
            bail!("'mkfifo' command doesn't work")
        }
    }
}

impl Drop for Channel {
    fn drop(&mut self) {
        let path = self.0.clone();
        let log_state = LogState::new().push("path", path.display().to_string());
        std::fs::remove_file(path)
            .unwrap_or_else(|e| log::error!("channel deleating error: {e:?}"));
        log::trace!("{log_state} channel deleted");
    }
}
