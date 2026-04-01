use super::{MappedRawMessage, RawMessage};
use crate::{prelude::*, short_slice_u8};
use invoker_auth::{Challenge, Solution};
pub enum Income {
    Challenge(Challenge),
    AuthVerdict(bool),
}
impl std::fmt::Debug for Income {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AuthVerdict(verdict) => {
                write!(f, "{}", if *verdict { "Approved" } else { "Denied" })
            }
            Self::Challenge(challenge) => f
                .debug_struct("Challenge")
                .field("data", &Box::<[u8]>::from(short_slice_u8(&*challenge)))
                .finish(),
        }
    }
}
impl super::Income for Income {
    fn from_raw(msg: MappedRawMessage) -> Result<Self> {
        Ok(match msg.ty() {
            "AUTH_VERDICT" => Self::AuthVerdict(msg.field_eq("VERDICT", "APPROVED")),
            "AUTH_CHALLENGE" => {
                let Some(data) = msg.data() else {
                    bail!("data not found");
                };
                Self::Challenge(Challenge::from(&*data))
            }
            command => {
                bail!("incomming websocket message: incorrect command: {command}");
            }
        })
    }
}

pub enum Outgo {
    ChallengeSolution(Solution),
}

impl std::fmt::Debug for Outgo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ChallengeSolution(data) => f
                .debug_struct("ChallengeSolution")
                .field("data", &Box::<[u8]>::from(short_slice_u8(data)))
                .finish(),
        }
    }
}

impl super::Outgo for Outgo {
    fn into_raw(self) -> RawMessage {
        match self {
            Outgo::ChallengeSolution(data) => {
                let mut body = RawMessage::new("AUTH");
                body.set_data(Box::from(&*data));
                body
            }
        }
    }
}
