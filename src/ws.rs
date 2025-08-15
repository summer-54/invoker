use anyhow::anyhow;
use tokio::sync::Mutex;

use crate::{
    Result,
    api::{income, outgo},
};

use tokio::net::TcpStream;

use futures::{
    SinkExt, StreamExt,
    stream::{SplitSink, SplitStream},
};
use tokio_websockets::{ClientBuilder, MaybeTlsStream, Message, WebSocketStream};

pub use http::Uri;
pub struct Service {
    read: Mutex<SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>>,
    write: Mutex<SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>,
}

impl Service {
    pub async fn from_uri(uri: Uri) -> Result<Service> {
        let (client, _) = ClientBuilder::from_uri(uri).connect().await?;
        let (write, read) = client.split();
        Ok(Self {
            write: Mutex::new(write),
            read: Mutex::new(read),
        })
    }
    pub async fn send(&self, msg: outgo::Msg) -> Result<()> {
        log::info!("sending: {msg:?}");
        self.write
            .lock()
            .await
            .send(Message::text(match msg {
                outgo::Msg::FullTaskVerdict { verdict, data } => {
                    format!("VERDICT {}\n{}\n", verdict.to_string(), data)
                }
                outgo::Msg::SubTaskVerdict {
                    subtask_id,
                    verdict,
                    data,
                } => format!(
                    "SUBTASK {subtask_id}\nVERDICT {}\n{}\n",
                    verdict.to_string(),
                    data
                ),
                outgo::Msg::Exited { code, data } => {
                    format!("EXITED {code}\n{}\n", data)
                }
                outgo::Msg::Error { msg } => format!("ERROR\n{msg}\n"),
                outgo::Msg::OpError { msg } => format!("OPERROR\n{msg}\n"),
            }))
            .await?;
        Ok(())
    }
    pub async fn recv(&self) -> Result<income::Msg> {
        loop {
            let data = &*self
                .read
                .lock()
                .await
                .next()
                .await
                .ok_or(anyhow!("websocket connection was closed"))??
                .into_payload();

            let Some(pos) = data.iter().position(|&b| b == ('\n' as u8)) else {
                continue;
            };

            let (command, data) = data.split_at(pos + 1);
            let command = String::from_utf8_lossy(command);

            let msg = match command.trim() {
                "START" => income::Msg::Start {
                    data: Box::from(data),
                },
                "STOP" => income::Msg::Stop,
                "CLOSE" => income::Msg::Close,
                command => {
                    log::error!("incorrect command in incomming websocket message: {command}");
                    continue;
                }
            };
            return Ok(msg);
        }
    }
}
