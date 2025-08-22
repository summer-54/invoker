use crate::{
    Result, anyhow,
    api::{income, outgo},
};
pub use http::Uri;
use uuid::Uuid;

use {
    futures::{
        SinkExt, StreamExt,
        stream::{SplitSink, SplitStream},
    },
    tokio::{net::TcpStream, sync::Mutex},
    tokio_websockets::{ClientBuilder, MaybeTlsStream, Message, WebSocketStream},
};

pub struct Service {
    read: Mutex<SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>>,
    write: Mutex<SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>,
}

impl Service {
    pub async fn from_uri(uri: Uri) -> Result<Service> {
        let (client, _) = ClientBuilder::from_uri(uri).connect().await?;
        let (write, read) = client.split();
        let this = Self {
            write: Mutex::new(write),
            read: Mutex::new(read),
        };
        let token = Uuid::new_v4();
        println!("login in invoker manager with token: {token}");
        this.send(outgo::Msg::Token(token)).await?;
        Ok(this)
    }
    pub async fn send(&self, msg: outgo::Msg) -> Result<()> {
        log::info!("sending: {msg:?}");
        self.write
            .lock()
            .await
            .send(Message::binary(match msg {
                outgo::Msg::FullVerdict(verdict) => match verdict {
                    outgo::FullVerdict::Ok {
                        score,
                        groups_score,
                    } => {
                        let mut ws_msg = format!("VERDICT OK\nSUM {score}\n GROUPS\n");
                        for score in groups_score {
                            ws_msg.push_str(&format!("{score}\n"));
                        }
                        ws_msg
                    }
                    outgo::FullVerdict::Ce(msg) => {
                        format!("VERDICT CE\n {msg}")
                    }
                    outgo::FullVerdict::Te(msg) => {
                        format!("VERDICT CE\n {msg}")
                    }
                }
                .into_bytes(),
                outgo::Msg::TestVerdict {
                    test_id,
                    verdict,
                    time,
                    memory,
                    data,
                } => {
                    let mut bin_msg = format!(
                        "TEST {test_id}\nVERDICT {verdict}\nTIME {time}\nMEMORY {memory}\n"
                    )
                    .into_bytes();
                    bin_msg.append(&mut data.into_vec());
                    bin_msg
                }
                outgo::Msg::Exited { code, data } => {
                    format!("EXITED {code}\n{}\n", data).into_bytes()
                }
                outgo::Msg::Error { msg } => format!("ERROR\n{msg}\n").into_bytes(),
                outgo::Msg::OpError { msg } => format!("OPERROR\n{msg}\n").into_bytes(),
                outgo::Msg::Token(token) => format!("TOKEN\n{}\n", token.as_u128()).into_bytes(),
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
