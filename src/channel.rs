use crate::prelude::*;
pub struct Channel(pub Box<str>);

impl Channel {
    pub async fn new(dir: &str) -> Result<Channel> {
        let id: u64 = rand::random();
        let path = format!("{dir}/{id}").into_boxed_str();

        let status = tokio::process::Command::new("mkfifo")
            .arg("-m")
            .arg("777")
            .arg(&*path)
            .status()
            .await?;
        if status.success() {
            log::trace!("new by path: {path}");

            Ok(Channel(path))
        } else {
            bail!("'mkfifo' command doesn't work")
        }
    }
}

impl Drop for Channel {
    fn drop(&mut self) {
        let path = self.0.clone();
        tokio::spawn(async move {
            log::trace!("deleated by path: {path}");
            tokio::fs::remove_file(path.to_string()).await
        });
    }
}
