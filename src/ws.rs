use std::collections::HashMap;

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

fn deserialize_msg(mut buf: &[u8]) -> (HashMap<Box<str>, Box<str>>, Option<Box<[u8]>>) {
    let mut map = HashMap::<Box<str>, Box<str>>::new();

    let data = loop {
        let Some(endl_pos) = buf.iter().position(|&b| b == ('\n' as u8)) else {
            break None;
        };
        let (line, other) = buf.split_at(endl_pos + 1);
        buf = other;

        let line = String::from_utf8_lossy(line);
        let (key, value) = line.split_once(' ').unwrap_or((&*line, ""));
        let key = key.trim();
        let value = value.trim();

        if key == "DATA" {
            break Some(buf.into());
        } else {
            map.insert(key.into(), value.into());
        }
    };

    (map, data)
}

fn serialize_msg(
    iter: impl std::iter::Iterator<Item = (Box<str>, Box<str>)>,
    data: Option<Box<[u8]>>,
) -> Box<[u8]> {
    let mut buf = vec![];
    for (k, v) in iter {
        buf.append(&mut format!("{k} {v}\n").as_bytes().to_vec());
    }
    if let Some(data) = data {
        buf.append(&mut "DATA\n".as_bytes().to_vec());
        buf.append(&mut data.to_vec());
    }
    buf.into_boxed_slice()
}

pub struct Service {
    read: Mutex<Receiver<TcpStream, DeflateDecoder>>,
    write: Mutex<Sender<TcpStream, DeflateEncoder>>,
}

impl Service {
    pub async fn new<A: ToSocketAddrs>(socket_addr: A, uri: Uri) -> Result<Service> {
        let stream = TcpStream::connect(socket_addr).await?;
        log::trace!("(socket_add)start subscribing");
        let client = subscribe_with(
            WebSocketConfig::default(),
            stream,
            uri,
            DeflateExtProvider::with_config(DeflateConfig::default()),
            SubprotocolRegistry::default(),
        )
        .await?;
        log::trace!("end subscribing");

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
                {
                    let mut map = Vec::<(Box<str>, Box<str>)>::new();
                    let mut msg_data = None;
                    match msg {
                        outgo::Msg::FullVerdict(verdict) => {
                            map.push(("TYPE".into(), "VERDICT".into()));
                            match verdict {
                                outgo::FullVerdict::Ok {
                                    score,
                                    groups_score,
                                } => {
                                    map.push(("VERDICT".into(), "OK".into()));
                                    map.push(("SUM".into(), format!("{score}").into()));
                                    map.push((
                                        "GROUPS".into(),
                                        String::from_utf8_lossy(
                                            &*groups_score
                                                .into_iter()
                                                .flat_map(|score| format!("{score} ").into_bytes())
                                                .collect::<Vec<u8>>(),
                                        )
                                        .into(),
                                    ));
                                }
                                outgo::FullVerdict::Ce(msg) => {
                                    map.push(("VERDICT".into(), "CE".into()));
                                    map.push(("MESSAGE".into(), msg));
                                }
                                outgo::FullVerdict::Te(msg) => {
                                    map.push(("VERDICT".into(), "CE".into()));
                                    map.push(("MESSAGE".into(), msg));
                                }
                            }
                        }
                        outgo::Msg::TestVerdict {
                            test_id,
                            verdict,
                            time,
                            memory,
                            data,
                        } => {
                            map.push(("TYPE".into(), "TEST".into()));
                            map.push(("ID".into(), format!("{test_id}").into()));
                            map.push(("VERDICT".into(), format!("{verdict}").into()));
                            map.push(("TIME".into(), format!("{time}").into()));
                            map.push(("MEMORY".into(), format!("{memory}").into()));
                            msg_data = Some(data);
                        }
                        outgo::Msg::Exited { code, data } => {
                            map.push(("TYPE".into(), "EXITED".into()));
                            map.push(("CODE".into(), format!("{code}").into()));
                            map.push(("MESSAGE".into(), data));
                        }
                        outgo::Msg::Error { msg } => {
                            map.push(("TYPE".into(), "ERROR".into()));
                            map.push(("MESSAGE".into(), msg));
                        }
                        outgo::Msg::OpError { msg } => {
                            map.push(("TYPE".into(), "OPERROR".into()));
                            map.push(("MESSAGE".into(), msg));
                        }
                        outgo::Msg::Token(token) => {
                            map.push(("TYPE".into(), "TOKEN".into()));
                            map.push(("ID".into(), format!("{}", token.as_u128()).into()));
                        }
                    }
                    serialize_msg(map.into_iter(), msg_data)
                },
                ratchet_rs::PayloadType::Binary,
            )
            .await?;
        Ok(())
    }

    pub async fn recv(&self) -> Result<income::Msg> {
        loop {
            let mut msg = bytes::BytesMut::new();
            self.read.lock().await.read(&mut msg).await?;

            let (map, data) = deserialize_msg(&*msg);
            let Some(msg_type) = map.get("TYPE") else {
                log::error!("field 'TYPE' not found");
                continue;
            };

            let msg = match &**msg_type {
                "START" => {
                    let Some(data) = data else {
                        log::error!("data not found");
                        continue;
                    };
                    income::Msg::Start { data }
                }
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
