//! Abstractions for returning data from handlers.
//!
//! No relation to IRC channels.

use std::sync::Arc;

/// Send halves of non-blocking channels
pub trait Sender {
    /// The type of values consumed by this channel.
    type Value;

    /// Attempts to send a value over the channel.
    ///
    /// This function must never block.
    fn send(self: Arc<Self>, value: Self::Value) -> SendCont<Self::Value>;

    /// Returns whether attempting to send a value may succeed.
    ///
    /// A return value of `false` means a future send operation is guaranteed to fail.
    /// A return value of `true` means a future send operation may or may not succeed.
    fn may_send(&self) -> bool {
        true
    }
}

/// The outcome of attempting to send a message via a [`Sender`].
#[derive(Clone)]
pub enum SendCont<T> {
    /// The message was sent.
    Sent(Arc<dyn Sender<Value = T>>),
    /// The message was sent, but the channel accepts no further messages.
    SentClosed,
    /// The message was not sent and the channel accepts no further messages.
    Closed,
}

/// The sender half of channel as provided to a handler.
pub struct SenderRef<'a, T> {
    pub(super) sender: &'a mut Option<Arc<dyn Sender<Value = T>>>,
    pub(super) flag: &'a mut bool,
}

impl<'a, T> SenderRef<'a, T> {
    /// Sends one value to the underlying channel.
    ///
    /// Returns `true` if the value sent successfully, otherwise returns `false`.
    /// This return value can often be safely ignored.
    pub fn send(&mut self, value: T) -> bool {
        if let Some(sender) = self.sender.take() {
            let result = sender.send(value);
            let success = !matches!(result, SendCont::Closed);
            *self.flag |= success;
            if let SendCont::Sent(cont) = result {
                *self.sender = Some(cont);
            }
            success
        } else {
            false
        }
    }
    /// Returns `false` if a later send operation is guaranteed to fail.
    pub fn can_send(&self) -> bool {
        self.sender.is_some()
    }
}

/// A [`Sender`] that is always closed.
#[derive(Clone, Copy, Debug, Default)]
pub struct ClosedSender<T>(std::marker::PhantomData<fn(T)>);

impl<T> Sender for ClosedSender<T> {
    type Value = T;

    fn send(self: Arc<Self>, _value: T) -> SendCont<Self::Value> {
        SendCont::Closed
    }

    fn may_send(&self) -> bool {
        false
    }
}

impl<T: 'static> Sender for std::sync::mpsc::Sender<T> {
    type Value = T;

    fn send(self: Arc<Self>, value: T) -> SendCont<Self::Value> {
        if std::sync::mpsc::Sender::send(&*self, value).is_ok() {
            SendCont::Sent(self)
        } else {
            SendCont::Closed
        }
    }
}

impl<T> Sender for std::sync::OnceLock<T> {
    type Value = T;

    fn send(self: Arc<Self>, value: Self::Value) -> SendCont<Self::Value> {
        if self.set(value).is_ok() {
            SendCont::SentClosed
        } else {
            SendCont::Closed
        }
    }
}

impl<T> Sender for std::cell::OnceCell<T> {
    type Value = T;

    fn send(self: Arc<Self>, value: Self::Value) -> SendCont<Self::Value> {
        if self.set(value).is_ok() {
            SendCont::SentClosed
        } else {
            SendCont::Closed
        }
    }
}

#[cfg(feature = "tokio")]
impl<T> Sender for tokio::sync::oneshot::Sender<T> {
    type Value = T;

    fn send(self: Arc<Self>, value: Self::Value) -> SendCont<Self::Value> {
        if let Some(sender) = Arc::into_inner(self) {
            if sender.send(value).is_ok() {
                return SendCont::SentClosed;
            }
        }
        SendCont::Closed
    }
}

#[cfg(feature = "tokio")]
impl<T: 'static> Sender for tokio::sync::mpsc::UnboundedSender<T> {
    type Value = T;

    fn send(self: Arc<Self>, value: T) -> SendCont<Self::Value> {
        if tokio::sync::mpsc::UnboundedSender::send(&*self, value).is_ok() {
            SendCont::Sent(self)
        } else {
            SendCont::Closed
        }
    }
}

#[cfg(feature = "tokio")]
impl<T: 'static> Sender for tokio::sync::mpsc::WeakUnboundedSender<T> {
    type Value = T;

    fn send(self: Arc<Self>, value: T) -> SendCont<Self::Value> {
        if let Some(sender) = self.upgrade() {
            if sender.send(value).is_ok() {
                return SendCont::Sent(self);
            }
        }
        SendCont::Closed
    }
}

/// Specifications for channel types.
///
/// All of the type members are considered to be the receiver side of the channel.
pub trait ChannelSpec {
    /// Channel that can be used up to once over its lifetime.
    type Oneshot<T>;
    /// Channel that is a non-blocking queue that can be used multiple times per message.
    type Queue<T>;

    /// Creates a new oneshot channel, the sender half of which is boxed.
    fn new_oneshot<T: 'static>(&self) -> (Arc<dyn Sender<Value = T>>, Self::Oneshot<T>);

    /// Creates a new queue channel, the sender half of which is boxed.
    fn new_queue<T: 'static>(&self) -> (Arc<dyn Sender<Value = T>>, Self::Queue<T>);
}

/// [`ChannelSpec`] for thread-safe synchronous channels.
pub struct SyncChannels;
#[cfg(feature = "tokio")]
/// [`ChannelSpec`] for Tokio channels.
pub struct TokioChannels;

impl ChannelSpec for SyncChannels {
    type Oneshot<T> = std::sync::Arc<std::sync::OnceLock<T>>;

    type Queue<T> = std::sync::mpsc::Receiver<T>;

    fn new_oneshot<T: 'static>(&self) -> (Arc<dyn Sender<Value = T>>, Self::Oneshot<T>) {
        let cell = std::sync::Arc::new(std::sync::OnceLock::new());
        let cellb = cell.clone();
        (cellb, cell)
    }

    fn new_queue<T: 'static>(&self) -> (Arc<dyn Sender<Value = T>>, Self::Queue<T>) {
        let (send, recv) = std::sync::mpsc::channel();
        (Arc::new(send), recv)
    }
}

#[cfg(feature = "tokio")]
impl ChannelSpec for TokioChannels {
    type Oneshot<T> = tokio::sync::oneshot::Receiver<T>;

    type Queue<T> = tokio::sync::mpsc::UnboundedReceiver<T>;

    fn new_oneshot<T: 'static>(&self) -> (Arc<dyn Sender<Value = T>>, Self::Oneshot<T>) {
        let (send, recv) = tokio::sync::oneshot::channel();
        (Arc::new(send), recv)
    }

    fn new_queue<T: 'static>(&self) -> (Arc<dyn Sender<Value = T>>, Self::Queue<T>) {
        let (send, recv) = tokio::sync::mpsc::unbounded_channel();
        (Arc::new(send), recv)
    }
}
