pub use http::Uri;

use crate::prelude::*;

use ratchet_rs::{
    Receiver, Sender, SubprotocolRegistry, UpgradedClient, WebSocketConfig,
    deflate::{DeflateConfig, DeflateDecoder, DeflateEncoder, DeflateExtProvider},
    subscribe_with,
};
use tokio::{
    net::{TcpStream, ToSocketAddrs},
    sync::{
        Mutex,
        mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel},
    },
};

use std::{collections::HashMap, sync::Arc};

use super::{
    RawMessage,
    stream::{self, Income, Outgo},
};

const MAX_MESSAGE_SIZE: usize = 1 << 31;

pub struct Stream {
    name: Box<str>,
    channel: Arc<Channel>,
    receiver: Mutex<UnboundedReceiver<RawMessage>>,
}

impl<I: Income, O: Outgo + Send> stream::Stream<I, O> for Stream {
    async fn recv(&self) -> Result<I> {
        I::from_raw(
            self.receiver
                .lock()
                .await
                .recv()
                .await
                .ok_or(anyhow!("receiver was closed"))?
                .into_mapped(),
        )
    }
    async fn send(&self, msg: O) -> Result<()> {
        Arc::clone(&self.channel)
            .send(self.name.clone(), msg.into_raw())
            .await
    }
}

pub struct Channel {
    streams: Mutex<HashMap<Box<str>, UnboundedSender<RawMessage>>>,
    read: Mutex<Receiver<TcpStream, DeflateDecoder>>,
    write: Mutex<Sender<TcpStream, DeflateEncoder>>,
}

impl Channel {
    pub async fn new<A: ToSocketAddrs>(socket_addr: A, uri: Uri) -> Result<Channel> {
        log::trace!("websocket start subscribing");
        let tcp_stream = TcpStream::connect(socket_addr)
            .await
            .context("TcpStream connecting")?;
        let client = subscribe_with(
            WebSocketConfig {
                max_message_size: MAX_MESSAGE_SIZE,
            },
            tcp_stream,
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

    pub async fn new_stream(self: &Arc<Self>, name: &str) -> Stream {
        let (sender, receiver) = unbounded_channel();
        let receiver = Mutex::new(receiver);
        let mut streams = self.streams.lock().await;
        streams.insert(Box::from(name), sender);
        Stream {
            name: Box::from(name),
            receiver,
            channel: Arc::clone(self),
        }
    }

    pub async fn run(self: Arc<Self>) -> Result<()> {
        loop {
            let mut buf = bytes::BytesMut::new();
            self.read
                .lock()
                .await
                .read(&mut buf)
                .await
                .context("reading websocket messages")?;
            let Some(endl_pos) = buf.iter().position(|&b| b == b'\n') else {
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

            sender.send(msg)?;
        }
    }
    async fn send(self: Arc<Self>, name: Box<str>, msg: RawMessage) -> Result<()> {
        self.write
            .lock()
            .await
            .write(
                name.bytes().chain(msg.into_bytes()).collect::<Box<[u8]>>(),
                ratchet_rs::PayloadType::Binary,
            )
            .await
            .context("websocket message sending")?;
        Result::<()>::Ok(())
    }
}
