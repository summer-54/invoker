use super::{MappedRawMessage, RawMessage};
use crate::{prelude::*, short_slice_u8};
pub enum Income {
    Package(Box<[u8]>),
}
impl std::fmt::Debug for Income {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Package(challenge) => f
                .debug_struct("Package")
                .field("data", &Box::<[u8]>::from(short_slice_u8(&*challenge)))
                .finish(),
        }
    }
}
impl super::Income for Income {
    fn from_raw(msg: MappedRawMessage) -> Result<Self> {
        Ok(match msg.ty() {
            "PACKAGE" => Self::Package(Box::from(msg.data().ok_or(anyhow!("data not found"))?)),
            command => {
                bail!("incomming websocket message: incorrect command: {command}");
            }
        })
    }
}

pub enum Outgo {
    Load { package_id: crate::file::Id },
}

impl std::fmt::Debug for Outgo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Load { package_id } => f.debug_struct("Load").field("id", package_id).finish(),
        }
    }
}

impl super::Outgo for Outgo {
    fn into_raw(self) -> RawMessage {
        match self {
            Outgo::Load { package_id } => {
                let mut body = RawMessage::new("LOAD");
                body.add_field(&"PACKAGE", &package_id);
                body
            }
        }
    }
}
