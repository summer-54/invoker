use crate::prelude::*;

use super::{
    MappedRawMessage, MultiplexChannel, RawMessage,
    stream::{Income, Outgo, Stream},
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
        sync::{
            Mutex,
            mpsc::{UnboundedSender, unbounded_channel},
        },
    },
};
const MAX_MESSAGE_SIZE: usize = 1 << 31;

pub struct Channel {
    streams: Mutex<HashMap<Box<str>, UnboundedSender<RawMessage>>>,
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
    async fn send(&self, stream: &str, body: RawMessage) -> Result<()> {
        log::info!("sending: [stream: {stream}] {body:?}");
        self.write
            .lock()
            .await
            .write(
                stream
                    .bytes()
                    .chain(body.into_bytes())
                    .collect::<Box<[u8]>>(),
                ratchet_rs::PayloadType::Binary,
            )
            .await
            .context("websocket message sending")?;
        Ok(())
    }
}

impl super::MultiplexChannel for Channel {
    async fn new_stream<I: Income, O: Outgo>(self: &Arc<Self>, name: &str) -> Stream<I, O, Self> {
        let (sender, receiver) = unbounded_channel();
        let mut streams = self.streams.lock().await;
        streams.insert(Box::from(name), sender);
        Stream::new(name, receiver, Arc::downgrade(&self))
    }
}

impl Channel {
    pub async fn run(self: Arc<Self>) -> Result<()> {
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
            let stream_name = String::from_utf8(buf[..endl_pos].into())?;
            let msg = match RawMessage::try_from(&buf[endl_pos..]) {
                Ok(msg) => msg,
                Err(err) => {
                    log::error!("parsing websockets: {err}");
                    continue;
                }
            };

            log::info!("received message: [stream_name: {stream_name}] {msg:?}");
            let streams = self.streams.lock().await;
            let Some(sender) = streams.get(&*stream_name) else {
                log::error!("unknown stream name: {stream_name}");
                continue;
            };

            sender.send(msg);
        }
    }
}
