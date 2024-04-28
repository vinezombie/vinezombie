pub mod channel;

use std::ops::ControlFlow;

use super::{
    queue::{Queue, QueueEditGuard},
    ClientState,
};
use crate::ircmsg::ServerMsg;

use channel::*;

/// Generic message handlers, typically intended to handle one expected batch of messages
/// and parse them into a more-useful form.
pub trait Handler: 'static + Send {
    /// The type of values produced by this handler.
    type Value: 'static;
    /// Processes one message.
    ///
    /// Returns [`ControlFlow::Break`] if this handler is finished processing messages.
    fn handle(
        &mut self,
        msg: &ServerMsg<'_>,
        state: &mut ClientState,
        queue: QueueEditGuard<'_>,
        channel: SenderRef<'_, Self::Value>,
    ) -> ControlFlow<()>;

    /// Returns `true` if this handler wants an owning message.
    ///
    /// Giving an owning message may be more performant if this handler
    /// clones the messages or their contents somewhere.
    fn wants_owning(&self) -> bool {
        false
    }
}

/// Marker indicating no handler was returned because none is needed.
///
/// This is used by some [`MakeHandler`] implementations that may not reasonably
/// expect a response from the server if certain capabilities are not in use,
/// usually `labeled-response` and `echo-message`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub struct NoHandler;

impl std::fmt::Display for NoHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "handler intentionally not created")
    }
}

impl std::error::Error for NoHandler {}

/// Converters for values into [`Handler`]s.
///
/// This exists to allow blanket conversions of certain types into handlers,
/// such as SASL authenticators.
pub trait MakeHandler<T> {
    /// The type of values yielded by the handler.
    type Value: 'static;
    /// The type of errors that can result while creating the handler.
    ///
    /// This is often `!` or [`NoHandler`].
    type Error;

    /// The type of the receiver half of the preferred channel type.
    type Receiver<Spec: ChannelSpec>;

    /// Converts `T` into a [`Handler`] and queues messages.
    fn make_handler(
        self,
        state: &ClientState,
        queue: QueueEditGuard<'_>,
        value: T,
    ) -> Result<Box<dyn Handler<Value = Self::Value>>, Self::Error>;

    /// Creates an instance of the preferred channel type for a given channel spec.
    ///
    /// This typically just calls one method of the provided [`ChannelSpec`].
    fn make_channel<Spec: ChannelSpec>(
        spec: &Spec,
    ) -> (Box<dyn Sender<Value = Self::Value> + Send>, Self::Receiver<Spec>);
}

/// Handlers that can be used directly without any further options.
pub trait SelfMadeHandler: Handler {
    /// The type of the receiver half of the preferred channel type.
    type Receiver<Spec: ChannelSpec>;

    /// Queues initial messages.
    fn queue_msgs(&self, state: &ClientState, queue: QueueEditGuard<'_>);

    /// Creates an instance of the preferred channel type for a given channel spec.
    ///
    /// This typically just calls one method of the provided [`ChannelSpec`].
    fn make_channel<Spec: ChannelSpec>(
        spec: &Spec,
    ) -> (Box<dyn Sender<Value = Self::Value> + Send>, Self::Receiver<Spec>);
}

impl<T: SelfMadeHandler> MakeHandler<T> for () {
    type Value = T::Value;

    type Error = std::convert::Infallible;

    type Receiver<Spec: ChannelSpec> = T::Receiver<Spec>;

    fn make_handler(
        self,
        state: &ClientState,
        queue: QueueEditGuard<'_>,
        handler: T,
    ) -> Result<Box<dyn Handler<Value = T::Value>>, Self::Error> {
        handler.queue_msgs(state, queue);
        Ok(Box::new(handler))
    }

    fn make_channel<Spec: ChannelSpec>(
        spec: &Spec,
    ) -> (Box<dyn Sender<Value = Self::Value> + Send>, Self::Receiver<Spec>) {
        T::make_channel(spec)
    }
}

type BoxHandler =
    Box<dyn FnMut(&ServerMsg<'_>, &mut ClientState, QueueEditGuard<'_>) -> HandlerStatus + Send>;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum HandlerStatus {
    Keep { yielded: bool, wants_owning: bool },
    Done { yielded: bool },
}

fn box_handler<T: 'static>(
    mut handler: Box<dyn Handler<Value = T>>,
    mut sender: Box<dyn Sender<Value = T> + Send>,
) -> BoxHandler {
    Box::new(move |msg, state, queue| {
        let mut yielded = false;
        let sr = SenderRef { sender: &mut *sender, flag: &mut yielded };
        if handler.handle(msg, state, queue, sr).is_break() {
            HandlerStatus::Done { yielded }
        } else {
            HandlerStatus::Keep { yielded, wants_owning: handler.wants_owning() }
        }
    })
}

pub(crate) struct Handlers {
    handlers: Vec<(BoxHandler, usize)>,
    yielded: Vec<usize>,
    finished: Vec<usize>,
    wants_owning: bool,
}

impl Default for Handlers {
    fn default() -> Self {
        Handlers {
            // 2 handlers minimum in most cases: AutoPong, and whatever else.
            handlers: Vec::with_capacity(2),
            yielded: Vec::new(),
            // Registration handler finishes, and will be used in most cases.
            finished: Vec::with_capacity(1),
            wants_owning: false,
        }
    }
}

impl Handlers {
    pub fn add<T: 'static>(
        &mut self,
        handler: Box<dyn Handler<Value = T>>,
        sender: Box<dyn Sender<Value = T> + Send>,
    ) -> usize {
        self.wants_owning |= handler.wants_owning();
        let id = self.finished.pop().unwrap_or(self.handlers.len());
        let boxed = box_handler(handler, sender);
        self.handlers.push((boxed, id));
        id
    }

    pub fn is_empty(&self) -> bool {
        self.handlers.is_empty()
    }

    #[allow(unused)]
    pub fn wants_owning(&self) -> bool {
        self.wants_owning
    }

    #[allow(unused)]
    pub fn cancel_one(&mut self, id: usize) {
        if let Some(idx) = self.handlers.iter().position(|(_, id2)| id == *id2) {
            self.handlers.swap_remove(idx);
            self.finished.push(id);
            self.wants_owning &= !self.handlers.is_empty();
        } else {
            panic!("attemped to cancel handler with invalid id")
        }
    }

    pub fn cancel(&mut self) {
        self.handlers.clear();
        self.finished.clear();
        self.yielded.clear();
        self.wants_owning = false;
    }

    pub fn has_results(&self, finished_at: usize) -> bool {
        !self.yielded.is_empty() || self.finished.len() > finished_at
    }

    pub fn last_run_results(&self, finished_at: usize) -> (&[usize], &[usize]) {
        let (_, finished) = self.finished.split_at(finished_at);
        (self.yielded.as_slice(), finished)
    }

    pub fn handle(
        &mut self,
        msg: &ServerMsg<'_>,
        state: &mut ClientState,
        queue: &mut Queue,
    ) -> usize {
        self.wants_owning = false;
        self.yielded.clear();
        let finished_at = self.finished.len();
        let mut i = 0usize;
        while let Some((handler, id)) = self.handlers.get_mut(i) {
            match (handler)(msg, state, queue.edit()) {
                HandlerStatus::Keep { yielded, wants_owning } => {
                    if yielded {
                        self.yielded.push(*id);
                    }
                    self.wants_owning |= wants_owning;
                    i += 1;
                }
                HandlerStatus::Done { yielded } => {
                    if yielded {
                        self.yielded.push(*id);
                    }
                    self.finished.push(*id);
                    let _ = self.handlers.swap_remove(i);
                }
            }
        }
        finished_at
    }
}

/// Helper function to strip the data from a [`ControlFlow`].
///
/// The `Try` impl of `ControlFlow` does not perform any conversions as of Rust 1.70.
/// This function serves as boilerplate reduction to make it easier to use the `?` operator
/// in handlers.
#[inline]
pub fn cf_discard<A, B>(cf: ControlFlow<A, B>) -> ControlFlow<()> {
    match cf {
        ControlFlow::Continue(_) => ControlFlow::Continue(()),
        ControlFlow::Break(_) => ControlFlow::Break(()),
    }
}
