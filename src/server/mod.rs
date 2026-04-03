#[cfg(feature = "mock")]
pub mod mock;
pub mod stream;
#[cfg(not(feature = "mock"))]
pub mod websocket;

use crate::prelude::*;
use crate::short_slice_u8;
use std::{collections::HashMap, sync::Arc};
#[derive(Debug)]
pub struct MappedRawMessage {
    map: HashMap<Arc<str>, usize>,
    msg: RawMessage,
}

pub struct RawMessage {
    pub(self) ty: Box<str>,
    pub(self) fields: Vec<(Arc<str>, Box<str>)>,
    pub(self) data: Option<Box<[u8]>>,
}
impl std::fmt::Debug for RawMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct(&*self.ty);
        s.field("fields", &self.fields);
        if let Some(data) = &self.data {
            s.field("data", &Box::<[u8]>::from(short_slice_u8(data)));
        }
        s.finish()
    }
}
impl TryFrom<&[u8]> for RawMessage {
    type Error = Error;

    fn try_from(mut buf: &[u8]) -> Result<Self> {
        let mut fields = Vec::<(Arc<str>, Box<str>)>::new();
        let mut ty = Option::<Box<str>>::None;

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
                "TYPE" => ty = Some(value.into()),
                _ => fields.push((key.into(), value.into())),
            }
        };

        let Some(ty) = ty else {
            bail!("cannot parse raw msg, field 'TYPE' not found")
        };

        Ok(Self {
            ty,
            fields: fields,
            data,
        })
    }
}

impl RawMessage {
    pub fn new(ty: impl ToString) -> Self {
        Self {
            ty: ty.to_string().into_boxed_str(),
            fields: vec![],
            data: None,
        }
    }
    pub fn into_bytes(self) -> impl Iterator<Item = u8> {
        let buf = format!("TYPE {}\n", self.ty).into_bytes().into_iter();
        let buf = buf
            .chain(
                self.fields
                    .into_iter()
                    .flat_map(|(k, v)| format!("{k} {v}\n").into_bytes()),
            )
            .chain(
                self.data
                    .into_iter()
                    .flat_map(|data| "DATA\n".bytes().chain(data.into_iter())),
            );
        buf
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
    pub fn into_mapped(self) -> MappedRawMessage {
        MappedRawMessage {
            map: self
                .fields
                .iter()
                .enumerate()
                .map(|(i, (k, _))| (Arc::clone(k), i))
                .collect(),
            msg: self,
        }
    }
}

impl From<RawMessage> for MappedRawMessage {
    fn from(value: RawMessage) -> MappedRawMessage {
        value.into_mapped()
    }
}

impl TryFrom<&[u8]> for MappedRawMessage {
    type Error = Error;
    fn try_from(value: &[u8]) -> Result<Self> {
        Ok(Self::from(RawMessage::try_from(value)?))
    }
}
impl MappedRawMessage {
    pub fn field(&self, name: &str) -> Option<&str> {
        Some(&*self.msg.fields[*self.map.get(name)?].1)
    }
    pub fn field_eq(&self, name: &str, value: &str) -> bool {
        let Some(field) = self.field(name) else {
            return false;
        };
        *field == *value
    }
    pub fn ty(&self) -> &str {
        &self.msg.ty
    }
    pub fn data(&self) -> Option<&[u8]> {
        self.msg.data.as_deref()
    }
}
