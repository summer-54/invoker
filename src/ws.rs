use crate::{
    Result,
    api::{income, outgo},
};

pub use http::Uri;

use {
    ratchet_rs::{
        Receiver, Sender, SubprotocolRegistry, UpgradedClient, WebSocketConfig,
        deflate::{DeflateConfig, DeflateDecoder, DeflateEncoder, DeflateExtProvider},
        subscribe_with,
    },
    tokio::{
        net::{TcpStream, ToSocketAddrs},
        sync::Mutex,
    },
};

pub struct Service {
    read: Mutex<Receiver<TcpStream, DeflateDecoder>>,
    write: Mutex<Sender<TcpStream, DeflateEncoder>>,
}

impl Service {
    pub async fn new<A: ToSocketAddrs>(socket_add: A, uri: Uri) -> Result<Service> {
        let stream = TcpStream::connect(socket_add).await?;
        let client = subscribe_with(
            WebSocketConfig::default(),
            stream,
            uri,
            DeflateExtProvider::with_config(DeflateConfig::default()),
            SubprotocolRegistry::default(),
        )
        .await?;
        let UpgradedClient {
            websocket,
            subprotocol,
        } = client;

        log::info!("websocket subprotocol: {subprotocol:?}");

        let (write, read) = websocket.split()?;
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
            .write(
                match msg {
                    outgo::Msg::FullVerdict(verdict) => match verdict {
                        outgo::FullVerdict::Ok {
                            score,
                            groups_score,
                        } => {
                            let mut ws_msg = format!("VERDICT OK\nSUM {score}\nGROUPS");
                            for score in groups_score {
                                ws_msg.push_str(&format!(" {score}"));
                            }
                            ws_msg.push('\n');
                            ws_msg
                        }
                        outgo::FullVerdict::Ce(msg) => {
                            format!("VERDICT CE\n {msg}\n")
                        }
                        outgo::FullVerdict::Te(msg) => {
                            format!("VERDICT CE\n {msg}\n")
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
                    outgo::Msg::Token(token) => {
                        format!("TOKEN\n{}\n", token.as_u128()).into_bytes()
                    }
                },
                ratchet_rs::PayloadType::Binary,
            )
            .await?;
        Ok(())
    }
    pub async fn recv(&self) -> Result<income::Msg> {
        loop {
            let mut data = bytes::BytesMut::new();
            self.read.lock().await.read(&mut data).await?;

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
                    log::error!("incomming websocket message: incorrect command: {command}");
                    continue;
                }
            };

            log::info!("recieved message: {msg:?}");
            return Ok(msg);
        }
    }
}
