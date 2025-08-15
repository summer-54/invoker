use tokio::sync::{
    Mutex,
    mpsc::{UnboundedReceiver, UnboundedSender},
};
pub struct Pull<I> {
    sender: UnboundedSender<I>,
    receiver: Mutex<UnboundedReceiver<I>>,
}

impl<I> FromIterator<I> for Pull<I> {
    fn from_iter<T: IntoIterator<Item = I>>(iter: T) -> Self {
        let mut iter = iter.into_iter();
        let this = Self::new();
        while let Some(i) = iter.next() {
            this.put(i);
        }
        this
    }
}

impl<T> Pull<T> {
    pub fn new() -> Self {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        Self {
            sender,
            receiver: Mutex::new(receiver),
        }
    }

    pub fn put(&self, t: T) {
        self.sender.send(t).unwrap();
    }

    pub async fn take(&self) -> T {
        self.receiver.lock().await.recv().await.unwrap()
    }
}
