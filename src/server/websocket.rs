use crate::prelude::*;

use super::{
    RawMessage,
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

impl Channel {
    pub async fn new_stream<I: Income + 'static, O: Outgo>(
        self: &Arc<Self>,
        name: &str,
    ) -> Stream<I, O> {
        let (sender, receiver) = unbounded_channel();
        let receiver = Arc::new(Mutex::new(receiver));
        let mut streams = self.streams.lock().await;
        streams.insert(Box::from(name), sender);
        let this = self.clone();
        let name_boxed = Box::<str>::from(name);
        Stream::new(
            Box::new(move |msg: O| {
                let name_clone = name_boxed.clone();
                log::info!("sending: [stream: {name_boxed}] {msg:?}");
                Box::pin(this.clone().send(name_clone, msg.into_raw()))
                    as super::stream::SendClosureResult
            }),
            Box::new(move || {
                let receiver_clone = receiver.clone();
                Box::pin(async move {
                    receiver_clone
                        .clone()
                        .lock()
                        .await
                        .recv()
                        .await
                        .map(|msg| I::from_raw(msg.into_mapped()))
                        .ok_or(anyhow!("websocket stream was closed"))?
                }) as super::stream::ReceiverClosureResult<I>
            }),
        )
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
