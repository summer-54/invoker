use crate::prelude::*;

use super::{income, outgo};

pub use http::Uri;
use invoker_auth::Challenge;

const MAX_MESSAGE_SIZE: usize = 1 << 31;

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

mod raw_msg {
    use crate::prelude::*;
    use std::{collections::HashMap, sync::Arc};
    pub struct Msg {
        map: HashMap<Arc<str>, usize>,
        body: Body,
    }
    pub struct Body {
        pub(self) r#type: Box<str>,
        pub(self) fields: Vec<(Arc<str>, Box<str>)>,
        pub(self) data: Option<Box<[u8]>>,
    }

    impl TryFrom<&[u8]> for Body {
        type Error = Error;

        fn try_from(mut buf: &[u8]) -> Result<Self> {
            let mut fields = Vec::<(Arc<str>, Box<str>)>::new();
            let mut r#type = Option::<Box<str>>::None;
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

                match key {
                    "DATA" => break Some(buf.into()),
                    "TPYE" => r#type = Some(value.into()),
                    _ => fields.push((key.into(), value.into())),
                }
            };

            let Some(r#type) = r#type else {
                bail!("cannot parse raw msg, field 'TYPE' not found")
            };

            Ok(Self {
                r#type,
                fields: fields,
                data,
            })
        }
    }

    impl Body {
        pub fn new(r#type: impl ToString) -> Self {
            Self {
                r#type: r#type.to_string().into_boxed_str(),
                fields: vec![],
                data: None,
            }
        }
        pub fn into_bytes(self) -> Box<[u8]> {
            let mut buf = format!("TYPE {}\n", self.r#type).as_bytes().to_vec();
            for (k, v) in self.fields {
                buf.append(&mut format!("{k} {v}\n").as_bytes().to_vec());
            }
            if let Some(data) = self.data {
                buf.append(&mut "DATA\n".as_bytes().to_vec());
                buf.append(&mut data.to_vec());
            }
            buf.into_boxed_slice()
        }

        pub fn add_field(&mut self, name: &dyn ToString, value: &dyn ToString) -> &mut Self {
            self.fields
                .push((name.to_string().into(), value.to_string().into()));
            self
        }
        pub fn add_fields(&mut self, fields: Vec<(&dyn ToString, &dyn ToString)>) -> &mut Self {
            for (name, value) in fields {
                self.add_field(name, value);
            }
            self
        }
        pub fn set_data(&mut self, data: Box<[u8]>) -> &mut Self {
            self.data = Some(data);
            self
        }
    }
    impl From<Body> for Msg {
        fn from(body: Body) -> Self {
            Msg {
                map: body
                    .fields
                    .iter()
                    .enumerate()
                    .map(|(i, (k, _))| (Arc::clone(k), i))
                    .collect(),
                body,
            }
        }
    }
    impl TryFrom<&[u8]> for Msg {
        type Error = Error;
        fn try_from(value: &[u8]) -> Result<Self> {
            Ok(Self::from(Body::try_from(value)?))
        }
    }
    impl Msg {
        pub fn field(&self, name: &str) -> Option<&str> {
            Some(&*self.body.fields[*self.map.get(name)?].1)
        }
        pub fn field_eq(&self, name: &str, value: &str) -> bool {
            let Some(field) = self.field(name) else {
                return false;
            };
            *field == *value
        }
        pub fn r#type(&self) -> &str {
            &self.body.r#type
        }
        pub fn data(&self) -> Option<&[u8]> {
            self.body.data.as_deref()
        }
    }
}
pub struct Service {
    read: Mutex<Receiver<TcpStream, DeflateDecoder>>,
    write: Mutex<Sender<TcpStream, DeflateEncoder>>,
}

impl Service {
    pub async fn new<A: ToSocketAddrs>(socket_addr: A, uri: Uri) -> Result<Service> {
        log::trace!("websocket start subscribing");
        let stream = TcpStream::connect(socket_addr)
            .await
            .context("TcpStream connecting")?;
        let client = subscribe_with(
            WebSocketConfig {
                max_message_size: MAX_MESSAGE_SIZE,
            },
            stream,
            uri,
            DeflateExtProvider::with_config(DeflateConfig::default()),
            SubprotocolRegistry::default(),
        )
        .await
        .context("connection subscribing")?;
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
}

impl outgo::Sender for Service {
    async fn send(&self, msg: outgo::Msg) -> Result<()> {
        log::info!("sending: {msg:?}");
        self.write
            .lock()
            .await
            .write(
                {
                    let body = match msg {
                        outgo::Msg::FullVerdict(verdict) => {
                            let mut body = raw_msg::Body::new("VERDICT");
                            match verdict {
                                outgo::FullVerdict::Ok {
                                    score,
                                    groups_score,
                                } => {
                                    body.add_fields(vec![
                                        (&"NAME", &"OK"),
                                        (&"SUM", &score),
                                        (
                                            &"GROUPS",
                                            &String::from_utf8_lossy(
                                                &*groups_score
                                                    .into_iter()
                                                    .flat_map(|score| {
                                                        format!("{score} ").into_bytes()
                                                    })
                                                    .collect::<Vec<u8>>(),
                                            ),
                                        ),
                                    ]);
                                }
                                outgo::FullVerdict::Ce(msg) => {
                                    body.add_fields(vec![(&"NAME", &"CE"), (&"MESSAGE", &msg)]);
                                }
                                outgo::FullVerdict::Te(msg) => {
                                    body.add_fields(vec![(&"NAME", &"TE"), (&"MESSAGE", &msg)]);
                                }
                            }
                            body
                        }
                        outgo::Msg::TestVerdict {
                            test_id,
                            verdict,
                            time,
                            memory,
                            data,
                        } => {
                            let mut body = raw_msg::Body::new("TEST");
                            body.add_fields(vec![
                                (&"ID", &test_id),
                                (&"VERDCIT", &verdict),
                                (&"TIME", &time),
                                (&"MEMORY", &memory),
                            ])
                            .set_data(data);
                            body
                        }
                        outgo::Msg::Exited { code, data } => {
                            let mut body = raw_msg::Body::new("EXITED");
                            body.add_fields(vec![(&"CODE", &code), (&"MESSAGE", &data)]);
                            body
                        }
                        outgo::Msg::Error { msg } => {
                            let mut body = raw_msg::Body::new("ERROR");
                            body.add_field(&"MESSAGE", &msg);
                            body
                        }
                        outgo::Msg::OpError { msg } => {
                            let mut body = raw_msg::Body::new("OPERROR");
                            body.add_field(&"MESSAGE", &msg);
                            body
                        }
                        outgo::Msg::Token { token, name } => {
                            let mut body = raw_msg::Body::new("TOKEN");
                            body.add_fields(vec![(&"ID", &token.as_u128()), (&"KEY", &name)]);
                            body
                        }
                        outgo::Msg::ChallengeSolution(data) => {
                            let mut body = raw_msg::Body::new("AUTH");
                            body.set_data(Box::from(&*data));
                            body
                        }
                    };
                    body.into_bytes()
                },
                ratchet_rs::PayloadType::Binary,
            )
            .await
            .context("websocket message sending")?;
        Ok(())
    }
}
impl income::Receiver for Service {
    async fn recv(&self) -> Result<income::Msg> {
        loop {
            let mut msg = bytes::BytesMut::new();
            self.read
                .lock()
                .await
                .read(&mut msg)
                .await
                .context("reading websocket messages")?;
            let msg = match raw_msg::Msg::try_from(&*msg) {
                Ok(msg) => msg,
                Err(err) => {
                    log::error!("parsing websockets: {err}");
                    continue;
                }
            };

            let msg = match msg.r#type() {
                "AUTH_VERDICT" => income::Msg::AuthVerdict(msg.field_eq("VERDICT", "APPROVED")),
                "AUTH_CHALLENGE" => {
                    let Some(data) = msg.data() else {
                        log::error!("data not found");
                        continue;
                    };
                    income::Msg::Challenge(Challenge::from(&*data))
                }
                "START" => {
                    let Some(data) = msg.data() else {
                        log::error!("data not found");
                        continue;
                    };
                    income::Msg::Start {
                        data: Box::from(data),
                    }
                }
                "STOP" => income::Msg::Stop,
                "CLOSE" => income::Msg::Close,
                command => {
                    log::error!("incomming websocket message: incorrect command: {command}");
                    continue;
                }
            };

            log::info!("received message: {msg:?}");
            return Ok(msg);
        }
    }
}
