#[cfg(not(feature = "mock"))]
pub mod websocket;

use crate::prelude::*;
use crate::short_slice_u8;
use std::marker::PhantomData;
use std::{
    collections::HashMap,
    sync::{Arc, Weak},
};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

#[derive(Debug)]
pub struct RawMessage {
    map: HashMap<Arc<str>, usize>,
    body: Body,
}

pub struct Body {
    pub(self) ty: Box<str>,
    pub(self) fields: Vec<(Arc<str>, Box<str>)>,
    pub(self) data: Option<Box<[u8]>>,
}
impl std::fmt::Debug for Body {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = f.debug_struct(&*self.ty).field("fields", &**self.fields);
        if let Some(data) = &self.data {
            s.field("data", &Box::<[u8]>::from(short_slice_u8(data)));
        }
        s.finish()
    }
}
impl TryFrom<&[u8]> for Body {
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

impl Body {
    pub fn new(ty: impl ToString) -> Self {
        Self {
            ty: ty.to_string().into_boxed_str(),
            fields: vec![],
            data: None,
        }
    }
    pub fn into_bytes(self) -> Box<[u8]> {
        let mut buf = format!("TYPE {}\n", self.ty).as_bytes().to_vec();
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
impl From<Body> for RawMessage {
    fn from(body: Body) -> Self {
        RawMessage {
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
impl TryFrom<&[u8]> for RawMessage {
    type Error = Error;
    fn try_from(value: &[u8]) -> Result<Self> {
        Ok(Self::from(Body::try_from(value)?))
    }
}
impl RawMessage {
    pub fn field(&self, name: &str) -> Option<&str> {
        Some(&*self.body.fields[*self.map.get(name)?].1)
    }
    pub fn field_eq(&self, name: &str, value: &str) -> bool {
        let Some(field) = self.field(name) else {
            return false;
        };
        *field == *value
    }
    pub fn ty(&self) -> &str {
        &self.body.ty
    }
    pub fn data(&self) -> Option<&[u8]> {
        self.body.data.as_deref()
    }
}

trait Message: TryFrom<RawMessage> + Into<Body> {}

struct Stream<M: Message, C: MultiplexChannel> {
    _pd: PhantomData<M>,
    name: Box<str>,
    receiver: UnboundedReceiver<Box<str>>,
    service: Weak<C>,
}

impl<M: Message, C: MultiplexChannel> Stream<M, C> {
    async fn send(&self, msg: M) -> Result<()> {
        self.service.upgrade()?.send(self.name, msg.into()).await;
    }
    async fn recv(&self) -> Result<M> {
        Ok(self.receiver.recv().await?.into())
    }
}

pub trait MultiplexChannel: Sender {
    fn new_stream<M: Message>(
        self: Arc<Self>,
        name: &str,
    ) -> impl Future<Output = Stream<M, Self>> + Send;
}

trait Sender: Send + Sync {
    fn send(&self, stream: Box<str>, body: Body) -> impl Future<Output = Result<()>> + Send;
}
