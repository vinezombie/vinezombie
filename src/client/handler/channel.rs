//! Abstractions for returning data from handlers,
//! as well as implementations of channels and synchronization that's missing from std.
//!
//! No relation to IRC channels.

use std::ops::ControlFlow;

pub mod oneshot;
pub mod parker;
#[cfg(test)]
mod tests;

/// Whether a value successfully sent over a channel.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Sent {
    /// The channel is closed and the value was lost.
    Closed,
    /// The value was sent successfully.
    Ok,
}

impl From<ControlFlow<Sent>> for Sent {
    fn from(value: ControlFlow<Sent>) -> Self {
        match value {
            ControlFlow::Continue(_) => Sent::Ok,
            ControlFlow::Break(v) => v,
        }
    }
}

/// Send halves of non-blocking channels.
pub trait Sender {
    /// The type of values consumed by this channel.
    type Value;

    /// Attempts to send a value over the channel.
    /// Returns [`ControlFlow::Continue`] on successful send and if more values can be sent.
    /// Returns [`ControlFlow::Break`] if the channel is closed after the send.
    ///
    /// This function must not block while waiting for the receiving end to get values,
    /// as it may be called from async contexts.
    fn send(&mut self, value: Self::Value) -> ControlFlow<Sent>;

    /// Returns whether attempting to send a value may succeed.
    ///
    /// A return value of `false` means a future send operation is guaranteed to fail.
    /// A return value of `true` means a future send operation may or may not succeed.
    fn may_send(&self) -> bool {
        true
    }
}

/// The sender half of channel as provided to a handler.
pub struct SenderRef<'a, T> {
    pub(super) sender: &'a mut dyn Sender<Value = T>,
    pub(super) flag: &'a mut bool,
}

impl<'a, T> SenderRef<'a, T> {
    /// Returns `true` if at least one send occurred.
    pub fn did_send(&self) -> bool {
        *self.flag
    }
    /// Sends one value to the underlying channel.
    ///
    /// Returns [`ControlFlow::Continue`] if more values can be sent,
    /// otherwise returns [`ControlFlow::Break`].
    /// This return value can often be safely ignored.
    pub fn send(&mut self, value: T) -> ControlFlow<Sent> {
        let result = self.sender.send(value);
        *self.flag |= !matches!(result, ControlFlow::Break(Sent::Closed));
        result
    }
    /// Returns `false` if a later send operation is guaranteed to fail.
    pub fn may_send(&self) -> bool {
        self.sender.may_send()
    }
}

/// A [`Sender`] that is always closed.
#[derive(Clone, Copy, Debug, Default)]
pub struct ClosedSender<T>(std::marker::PhantomData<fn(T)>);

impl<T> Sender for ClosedSender<T> {
    type Value = T;

    fn send(&mut self, _value: T) -> ControlFlow<Sent> {
        ControlFlow::Break(Sent::Closed)
    }

    fn may_send(&self) -> bool {
        false
    }
}

impl<T> Sender for std::sync::mpsc::Sender<T> {
    type Value = T;

    fn send(&mut self, value: T) -> ControlFlow<Sent> {
        if std::sync::mpsc::Sender::send(&*self, value).is_ok() {
            ControlFlow::Continue(())
        } else {
            ControlFlow::Break(Sent::Closed)
        }
    }
}

#[cfg(feature = "tokio")]
impl<T> Sender for Option<tokio::sync::oneshot::Sender<T>> {
    type Value = T;

    fn send(&mut self, value: Self::Value) -> ControlFlow<Sent> {
        if let Some(sender) = self.take() {
            if sender.send(value).is_ok() {
                return ControlFlow::Break(Sent::Ok);
            }
        }
        ControlFlow::Break(Sent::Closed)
    }
}

#[cfg(feature = "tokio")]
impl<T> Sender for tokio::sync::mpsc::UnboundedSender<T> {
    type Value = T;

    fn send(&mut self, value: T) -> ControlFlow<Sent> {
        if tokio::sync::mpsc::UnboundedSender::send(&*self, value).is_ok() {
            ControlFlow::Continue(())
        } else {
            ControlFlow::Break(Sent::Closed)
        }
    }
}

#[cfg(feature = "tokio")]
impl<T> Sender for tokio::sync::mpsc::WeakUnboundedSender<T> {
    type Value = T;

    fn send(&mut self, value: T) -> ControlFlow<Sent> {
        if let Some(sender) = self.upgrade() {
            if sender.send(value).is_ok() {
                return ControlFlow::Continue(());
            }
        }
        ControlFlow::Break(Sent::Closed)
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
    fn new_oneshot<T: 'static + Send>(
        &self,
    ) -> (Box<dyn Sender<Value = T> + Send>, Self::Oneshot<T>);

    /// Creates a new queue channel, the sender half of which is boxed.
    fn new_queue<T: 'static + Send>(&self) -> (Box<dyn Sender<Value = T> + Send>, Self::Queue<T>);
}

/// [`ChannelSpec`] for thread-safe synchronous channels.
pub struct SyncChannels;
#[cfg(feature = "tokio")]
/// [`ChannelSpec`] for Tokio channels.
pub struct TokioChannels;

impl ChannelSpec for SyncChannels {
    type Oneshot<T> = (self::oneshot::Receiver<T>, self::parker::Parker);

    type Queue<T> = std::sync::mpsc::Receiver<T>;

    fn new_oneshot<T: 'static + Send>(
        &self,
    ) -> (Box<dyn Sender<Value = T> + Send>, Self::Oneshot<T>) {
        let (send, recv) = self::oneshot::channel();
        let (unparker, parker) = self::parker::new(Some(send));
        (Box::new(unparker), (recv, parker))
    }

    fn new_queue<T: 'static + Send>(&self) -> (Box<dyn Sender<Value = T> + Send>, Self::Queue<T>) {
        let (send, recv) = std::sync::mpsc::channel();
        (Box::new(send), recv)
    }
}

#[cfg(feature = "tokio")]
impl ChannelSpec for TokioChannels {
    type Oneshot<T> = tokio::sync::oneshot::Receiver<T>;

    type Queue<T> = tokio::sync::mpsc::UnboundedReceiver<T>;

    fn new_oneshot<T: 'static + Send>(
        &self,
    ) -> (Box<dyn Sender<Value = T> + Send>, Self::Oneshot<T>) {
        let (send, recv) = tokio::sync::oneshot::channel();
        (Box::new(Some(send)), recv)
    }

    fn new_queue<T: 'static + Send>(&self) -> (Box<dyn Sender<Value = T> + Send>, Self::Queue<T>) {
        let (send, recv) = tokio::sync::mpsc::unbounded_channel();
        (Box::new(send), recv)
    }
}
