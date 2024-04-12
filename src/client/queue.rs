pub mod adjuster;

use crate::ircmsg::{ClientMsg, ServerMsg};
use crate::string::{Key, NoNul};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// A rate-limited queue for client messages.
///
/// RFC 1459 recommends sending messages as a client in bursts of up to 5 messages,
/// followed by one message every 2 seconds.
/// This type enforces that recommendation by resticting how frequently
/// messages can be removed from it.
pub struct Queue {
    queue: VecDeque<ClientMsg<'static>>,
    delay: Duration,
    sub: Duration,
    timepoint: Instant,
    // TODO: Bespoke trait for this. We want Clone back.
    labeler: Option<Box<dyn FnMut() -> NoNul<'static> + Send>>,
}

impl std::fmt::Debug for Queue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut f = f.debug_struct("Queue");
        f.field("queue", &self.queue)
            .field("delay", &self.delay)
            .field("sub", &self.sub)
            .field("timepoint", &self.timepoint)
            .field("labeler", &self.labeler.is_some())
            .finish()
    }
}

impl Default for Queue {
    fn default() -> Self {
        Self::new()
    }
}
impl FromIterator<ClientMsg<'static>> for Queue {
    fn from_iter<T: IntoIterator<Item = ClientMsg<'static>>>(iter: T) -> Self {
        Self::from_queue(iter.into_iter().collect())
    }
}
impl From<Vec<ClientMsg<'static>>> for Queue {
    fn from(value: Vec<ClientMsg<'static>>) -> Self {
        Self::from_queue(value.into())
    }
}
impl From<VecDeque<ClientMsg<'static>>> for Queue {
    fn from(value: VecDeque<ClientMsg<'static>>) -> Self {
        Self::from_queue(value)
    }
}

impl Queue {
    /// Creates a new queue with the default rate limit.
    pub fn new() -> Self {
        Self::from_queue(VecDeque::with_capacity(4))
    }
    fn from_queue(queue: VecDeque<ClientMsg<'static>>) -> Self {
        Queue {
            queue,
            delay: Duration::from_secs(2),
            sub: Duration::from_secs(8),
            timepoint: Instant::now(),
            labeler: None,
        }
    }

    /// Returns `true` if no messages in the queue.
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
    /// Returns how many messages are in the queue.
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Changes the rate limit.
    ///
    /// `delay` specifies how much time should pass between messages.
    /// `burst` specifies how many additional messages may be sent during an initial burst,
    /// e.g. a value of `4` results in a burst of five messages.
    pub fn set_rate_limit(&mut self, delay: Duration, burst: u32) -> &mut Self {
        self.delay = delay;
        self.sub = delay.saturating_mul(burst);
        // Pessimistically sets the next-message delay
        // to the longest possible under the new settings.
        let now = Instant::now();
        self.timepoint = now.checked_add(self.sub.saturating_add(delay)).unwrap_or(now);
        self
    }
    /// Retrieves a message from the queue, subject to rate limits.
    ///
    /// If this function does not return a message,
    /// `timeout_fn` is called with the duration until the next message will be available,
    /// or `None` if the queue is empty.
    /// The duration is guaranteed to be non-zero. This can be used to adjust read timeouts.
    pub fn pop(&mut self, timeout_fn: impl FnOnce(Option<Duration>)) -> Option<ClientMsg<'static>> {
        if let Some(value) = self.queue.pop_front() {
            let mut delay = self.timepoint.saturating_duration_since(Instant::now());
            delay = delay.saturating_sub(self.sub);
            if delay.is_zero() {
                self.timepoint = std::cmp::max(self.timepoint, Instant::now()) + self.delay;
                Some(value)
            } else {
                self.queue.push_front(value);
                timeout_fn(Some(delay));
                None
            }
        } else {
            timeout_fn(None);
            None
        }
    }
    /// Updates messages in the queue based on an incoming message.
    pub fn adjust<F: adjuster::Adjuster + ?Sized>(
        &mut self,
        msg: &ServerMsg<'_>,
        adjuster: &mut F,
    ) {
        if adjuster.should_adjust(msg) {
            self.queue.retain_mut(|cmsg| adjuster.update(cmsg));
        }
    }

    /// Adds `label` tags to outgoing messages for `labeled-response`.
    ///
    /// Returns `None` is no labeler is configured for the underlying queue.
    pub fn use_labeler(&mut self, f: impl FnMut() -> NoNul<'static> + 'static + Send) -> &mut Self {
        self.labeler = Some(Box::new(f));
        self
    }

    /// Returns `true` if a `label` tag will be attached to outgoing messages.
    pub fn is_using_labeler(&self) -> bool {
        self.labeler.is_some()
    }

    /// Stops `label` tags from being added to outgoing messages.
    pub fn use_no_labeler(&mut self) -> &mut Self {
        self.labeler = None;
        self
    }

    /// Create an interface for adding messages to the queue.
    pub fn edit(&mut self) -> QueueEditGuard<'_> {
        let orig_len = self.queue.len();
        QueueEditGuard { queue: self, orig_len }
    }

    /// Discards all messages from the queue.
    pub fn clear(&mut self) {
        self.queue.clear();
    }

    /// Resets the queue's state.
    ///
    /// Clears all messages, resets the message delay tracking, and unsets the labeler.
    pub fn reset(&mut self) {
        self.clear();
        self.use_no_labeler();
        self.timepoint = Instant::now();
    }
}

/// Interface to a [`Queue`] that allows adding messages.
pub struct QueueEditGuard<'a> {
    queue: &'a mut Queue,
    orig_len: usize,
}

impl QueueEditGuard<'_> {
    /// Adds a message onto the end of a queue.
    pub fn push(&mut self, msg: ClientMsg<'static>) {
        self.queue.queue.push_back(msg);
    }

    /// Labels a message and pushes it, returning the label (if any).
    pub fn push_labeled(&mut self, mut msg: ClientMsg<'static>) -> Option<NoNul<'static>> {
        let label = self.queue.labeler.as_deref_mut().map(|labeler| {
            let label = labeler();
            msg.tags.edit().insert_pair(Key::from_str("label"), label.clone());
            label
        });
        self.push(msg);
        label
    }

    /// Returns `true` if no messages have been added using `self`.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns how many messages have been added to the queue over `self`'s lifetime.
    pub fn len(&self) -> usize {
        self.queue.queue.len() - self.orig_len
    }

    /// Discard all messages that have been added using `self`.
    pub fn clear(&mut self) -> &mut Self {
        self.queue.queue.truncate(self.orig_len);
        self
    }
}
