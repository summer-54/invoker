use crate::prelude::*;

use super::{
    Body, Message, MultiplexChannel, RawMessage, Stream, UnboundedSender, unbounded_channel,
};

pub use http::Uri;
use std::{collections::HashMap, sync::Arc};

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
const MAX_MESSAGE_SIZE: usize = 1 << 31;

pub struct Channel {
    streams: Mutex<HashMap<Box<str>, UnboundedSender<Box<str>>>>,
    read: Mutex<Receiver<TcpStream, DeflateDecoder>>,
    write: Mutex<Sender<TcpStream, DeflateEncoder>>,
}

impl Channel {
    pub async fn new<A: ToSocketAddrs>(socket_addr: A, uri: Uri) -> Result<Channel> {
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
            streams: Mutex::new(HashMap::new()),
            write: Mutex::new(write),
            read: Mutex::new(read),
        })
    }
}

impl super::Sender for Channel {
    async fn send(&self, stream: Box<str>, body: Body) -> Result<()> {
        log::info!("sending: [stream: {stream}] {body:?}");
        self.write
            .lock()
            .await
            .write(body.into_bytes(), ratchet_rs::PayloadType::Binary)
            .await
            .context("websocket message sending")?;
        Ok(())
    }
}

impl MultiplexChannel for Channel {
    async fn new_stream<M: Message>(self: Arc<Self>, name: &str) -> Stream<M, Self> {
        let (sender, receiver) = unbounded_channel();
        self.streams.lock().await.insert(Box::from(name), sender);
        Stream {
            _pd: Default::default(),
            name: Box::from(name),
            receiver: receiver,
            service: Arc::downgrade(&self),
        }
    }
}

impl Channel {
    async fn recv(&self) -> Result<(Box<str>, RawMessage)> {
        loop {
            let mut buf = bytes::BytesMut::new();
            self.read
                .lock()
                .await
                .read(&mut buf)
                .await
                .context("reading websocket messages")?;
            let Some(endl_pos) = buf.iter().position(|&b| b == ('\n' as u8)) else {
                log::error!("message does not have any endl, so stream name cant be readed");
                continue;
            };
            let stream_name = String::from_utf8(buf[..endl_pos].into());
            let msg = match RawMessage::try_from(&*buf[endl_pos..]) {
                Ok(msg) => msg,
                Err(err) => {
                    log::error!("parsing websockets: {err}");
                    continue;
                }
            };

            log::info!("received message: {msg:?}");
            return Ok(msg);
        }
    }
}
