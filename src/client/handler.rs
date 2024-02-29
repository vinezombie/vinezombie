pub mod channel;

use super::{Queue, QueueEditGuard};
use crate::ircmsg::ServerMsg;
use std::sync::Arc;

use channel::*;

/// Generic message handlers, typically intended to handle one expected batch of messages
/// and parse them into a more-useful form.
pub trait Handler: 'static {
    /// The type of values produced by this handler.
    type Value: 'static;
    /// Processes one message.
    ///
    /// Returns `true` if this handler is finished processing messages.
    fn handle(
        &mut self,
        msg: &ServerMsg<'_>,
        queue: QueueEditGuard<'_>,
        channel: SenderRef<'_, Self::Value>,
    ) -> bool;

    /// Returns `true` if this handler wants an owning message.
    ///
    /// Giving an owning message may be more performant if this handler
    /// clones the messages or their contents somewhere.
    fn wants_owning(&self) -> bool {
        false
    }
}

/// No handler was returned because none is needed.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct NoHandler;

impl std::fmt::Display for NoHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "handler intentionally not created")
    }
}

impl std::error::Error for NoHandler {}

/// Converters for values into [`Handler`]s.
///
/// This exists to allow a reusable set of options to be used
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
        &self,
        queue: QueueEditGuard<'_>,
        value: T,
    ) -> Result<impl Handler<Value = Self::Value>, Self::Error>;

    /// Creates an instance of the preferred channel type for a given channel spec.
    ///
    /// This typically just calls one method of the provided [`ChannelSpec`].
    fn make_channel<Spec: ChannelSpec>(
        spec: &Spec,
    ) -> (Arc<dyn Sender<Value = Self::Value>>, Self::Receiver<Spec>);
}

type BoxHandler = Box<dyn FnMut(&ServerMsg<'_>, QueueEditGuard<'_>) -> HandlerStatus>;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum HandlerStatus {
    Keep { yielded: bool, wants_owning: bool },
    Done { yielded: bool },
}

fn box_handler<T: 'static>(
    mut handler: impl Handler<Value = T>,
    sender: Arc<dyn Sender<Value = T>>,
) -> BoxHandler {
    let mut sender = Some(sender);
    Box::new(move |msg, queue| {
        let mut yielded = false;
        let sr = SenderRef { sender: &mut sender, flag: &mut yielded };
        if handler.handle(msg, queue, sr) {
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
            handlers: Vec::with_capacity(1),
            yielded: Vec::new(),
            finished: Vec::with_capacity(1),
            wants_owning: false,
        }
    }
}

impl Handlers {
    pub fn add<T: 'static>(
        &mut self,
        handler: impl Handler<Value = T>,
        sender: Arc<dyn Sender<Value = T>>,
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

    #[allow(unused)]
    pub fn cancel(&mut self) {
        self.handlers.clear();
        self.finished.clear();
        self.yielded.clear();
        self.wants_owning = false;
    }

    pub fn has_results(&self, finished_at: usize) -> bool {
        !self.yielded.is_empty() && self.finished.len() > finished_at
    }

    pub fn last_run_results(&self, finished_at: usize) -> (&[usize], &[usize]) {
        let (_, finished) = self.finished.split_at(finished_at);
        (self.yielded.as_slice(), finished)
    }

    pub fn handle(&mut self, msg: &ServerMsg<'_>, queue: &mut Queue) -> usize {
        self.wants_owning = false;
        self.yielded.clear();
        let finished_at = self.finished.len();
        let mut i = 0usize;
        while let Some((handler, id)) = self.handlers.get_mut(i) {
            match (handler)(msg, queue.edit()) {
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
