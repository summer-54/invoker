use crate::server::RawMessage;

use super::{
    MultiplexChannel,
    stream::{Income, Outgo, SendResult, Stream},
};
use std::sync::Arc;
pub struct MockChannel;

pub struct MockReceiver;
impl tokio_stream::Stream for MockReceiver {
    type Item = RawMessage;
    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        std::task::Poll::Pending
    }
}

impl MultiplexChannel for MockChannel {
    type Receiver = MockReceiver;
    async fn new_stream<I: Income, O: Outgo>(self: &Arc<Self>, _name: &str) -> Stream<I, O, Self> {
        Stream::new(
            MockReceiver,
            Arc::clone(self),
            Box::new(move |_msg, _this| {
                Box::pin(async move {
                    println!("mock");
                    Ok(())
                }) as SendResult
            }),
        )
    }
}
