use crate::{
    judge::{Lang, api::test::Verdict},
    prelude::*,
    short_slice_u8,
};

use super::{MappedRawMessage, RawMessage};
#[derive(Debug)]
pub enum FullVerdict {
    Ok {
        score: usize,
        groups_score: Box<[usize]>,
    },
    Ce(Box<str>),
    Te(Box<str>),
}
pub enum Income {
    Start {
        package_id: crate::file::Id,
        lang: crate::judge::Lang,
        data: Box<[u8]>,
    },
    Stop,
    Close,
}

impl std::fmt::Debug for Income {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Start {
                lang,
                package_id,
                data,
            } => f
                .debug_struct("Start")
                .field("package_id", package_id)
                .field("lang", lang)
                .field("data", &Box::<[u8]>::from(short_slice_u8(&data)))
                .finish(),
            Self::Stop => write!(f, "Stop"),
            Self::Close => write!(f, "Close"),
        }
    }
}

impl super::Income for Income {
    fn from_raw(msg: MappedRawMessage) -> Result<Self> {
        Ok(match msg.ty() {
            "START" => {
                let Some(data) = msg.data() else {
                    bail!("data not found");
                };
                let Some(lang) = msg.field("LANG") else {
                    bail!("LANG field not found");
                };
                let Some(package_id) = msg.field("PACKAGE") else {
                    bail!("PACKAGE field not found");
                };
                Self::Start {
                    package_id: package_id.parse()?,
                    lang: Lang::try_from(lang)?,
                    data: Box::from(data),
                }
            }
            "STOP" => Self::Stop,
            "CLOSE" => Self::Close,
            command => {
                bail!("incomming websocket message: incorrect command: {command}");
            }
        })
    }
}

#[allow(dead_code)]
pub enum Outgo {
    Token {
        token: uuid::Uuid,
        name: Box<str>,
    },
    FullVerdict(FullVerdict),
    TestVerdict {
        test_id: usize,
        verdict: Verdict,
        time: f64,
        memory: u64,
        data: Box<[u8]>,
    },
    Exited {
        code: u8,
        data: Box<str>,
    },
    Error {
        msg: Box<str>,
    },
    OpError {
        msg: Box<str>,
    },
}

impl std::fmt::Debug for Outgo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Token { token, name } => f
                .debug_struct("Token")
                .field("token", token)
                .field("name", name)
                .finish(),

            Self::FullVerdict(verdict) => f.debug_tuple("FullVerdict").field(verdict).finish(),
            Self::TestVerdict {
                test_id,
                verdict,
                time,
                memory,
                data,
            } => f
                .debug_struct("TestVerdict")
                .field("test_id", test_id)
                .field("verdict", verdict)
                .field("time", time)
                .field("memory", memory)
                .field("data", &Box::<[u8]>::from(short_slice_u8(data)))
                .finish(),
            Self::Exited { code, data } => f
                .debug_struct("Exited")
                .field("code", code)
                .field("data", data)
                .finish(),
            Self::Error { msg } => f.debug_struct("Error").field("msg", msg).finish(),
            Self::OpError { msg } => f.debug_struct("OpError").field("msg", msg).finish(),
        }
    }
}
impl super::Outgo for Outgo {
    fn into_raw(self) -> RawMessage {
        match self {
            Self::FullVerdict(verdict) => {
                let mut body = RawMessage::new("VERDICT");
                match verdict {
                    FullVerdict::Ok {
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
                                        .flat_map(|score| format!("{score} ").into_bytes())
                                        .collect::<Vec<u8>>(),
                                ),
                            ),
                        ]);
                    }
                    FullVerdict::Ce(msg) => {
                        body.add_fields(vec![(&"NAME", &"CE"), (&"MESSAGE", &msg)]);
                    }
                    FullVerdict::Te(msg) => {
                        body.add_fields(vec![(&"NAME", &"TE"), (&"MESSAGE", &msg)]);
                    }
                }
                body
            }
            Self::TestVerdict {
                test_id,
                verdict,
                time,
                memory,
                data,
            } => {
                let mut body = RawMessage::new("TEST");
                body.add_fields(vec![
                    (&"ID", &test_id),
                    (&"VERDCIT", &verdict),
                    (&"TIME", &time),
                    (&"MEMORY", &memory),
                ])
                .set_data(data);
                body
            }
            Self::Exited { code, data } => {
                let mut body = RawMessage::new("EXITED");
                body.add_fields(vec![(&"CODE", &code), (&"MESSAGE", &data)]);
                body
            }
            Self::Error { msg } => {
                let mut body = RawMessage::new("ERROR");
                body.add_field(&"MESSAGE", &msg);
                body
            }
            Self::OpError { msg } => {
                let mut body = RawMessage::new("OPERROR");
                body.add_field(&"MESSAGE", &msg);
                body
            }
            Self::Token { token, name } => {
                let mut body = RawMessage::new("TOKEN");
                body.add_fields(vec![(&"ID", &token.as_u128()), (&"KEY", &name)]);
                body
            }
        }
    }
}
