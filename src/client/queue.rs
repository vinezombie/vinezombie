//! The rate-limited queue and supporting definitions.
//!
//!
//! RFC 1459 recommends sending messages as a client in bursts of up to 5 messages,
//! followed by one message every 2 seconds.
//! The contents of this module enforce that recommendation by resticting how frequently
//! messages can be removed from it.

use crate::ircmsg::{ClientMsg, ServerMsg};
use crate::string::{Key, NoNul, User};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// A rate-limited queue for client messages.
///
/// See [module-level documentation][self] for more info.
pub struct Queue {
    queue: VecDeque<ClientMsg<'static>>,
    delay: Duration,
    sub: Duration,
    timepoint: Instant,
    // TODO: Bespoke trait for this.
    labeler: Option<Box<dyn FnMut() -> NoNul<'static> + Send>>,
    adjuster: Option<Box<dyn Adjuster>>,
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
            adjuster: None,
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
    pub fn adjust(&mut self, msg: &ServerMsg<'_>) {
        if let Some(adj) = self.adjuster.as_mut() {
            if adj.should_adjust(msg) {
                self.queue.retain_mut(|cmsg| adj.update(cmsg));
            }
        }
    }
    /// Uses the provided [`Adjuster`] to update the queue on incoming messages.
    pub fn use_adjuster(&mut self, adjuster: impl Adjuster + 'static) -> &mut Self {
        self.adjuster = Some(Box::new(adjuster));
        self
    }
    /// Removes the [`Adjuster`] for this queue.
    pub fn use_no_adjuster(&mut self) -> &mut Self {
        self.adjuster = None;
        self
    }

    /// Sets the provided function as the labeler for this queue,
    /// allowing users of [`QueueEditGuard`] to attach `label` tags to outgoing messages without
    /// having to edit the messages themselves.
    ///
    /// A labeler implies that `labeled-response` is in effect for whatever pops from this queue.
    /// Anything that pushes to this queue (some handlers) can test for the labeler's presence
    /// and may radically change its behavior (e.g. PRIVMSG handlers additionally waiting for
    /// a response if `echo-message` is also in effect).
    pub fn use_labeler(
        &mut self,
        labeler: impl FnMut() -> NoNul<'static> + 'static + Send,
    ) -> &mut Self {
        self.labeler = Some(Box::new(labeler));
        self
    }
    /// Uses a reasonable default labeler for this queue.
    ///
    /// See [`use_labeler`][Queue::use_labeler] for IMPORTANT caveats.
    pub fn use_labeler_default(&mut self) -> &mut Self {
        let mut id = 0u32;
        self.use_labeler(move || {
            id = id.overflowing_add(1).0;
            // TODO: Nope. Base64-encode.
            User::from_id(id).into()
        })
    }
    /// Removes the labeler for this queue.
    ///
    /// See [`use_labeler`][Queue::use_labeler] for IMPORTANT caveats.
    pub fn use_no_labeler(&mut self) -> &mut Self {
        self.labeler = None;
        self
    }
    /// Returns `true` if a labeler is present.
    pub fn is_using_labeler(&self) -> bool {
        self.labeler.is_some()
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
        if let Some(adjuster) = self.adjuster.as_mut() {
            adjuster.reset();
        }
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

    /// Returns `true` if a labeler is present.
    pub fn is_using_labeler(&self) -> bool {
        self.queue.labeler.is_some()
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

    /// Returns a new [`QueueEditGuard`] borrowing from `self`.
    ///
    /// After the guard is dropped, `Self`
    pub fn edit(&mut self) -> QueueEditGuard<'_> {
        let orig_len = self.queue.len();
        QueueEditGuard { queue: self.queue, orig_len }
    }
}

impl Extend<ClientMsg<'static>> for Queue {
    fn extend<T: IntoIterator<Item = ClientMsg<'static>>>(&mut self, iter: T) {
        self.queue.extend(iter);
    }
}

/// Logic for adjusting a [`Queue`] based on incoming messages.
//
// This exists to allow queued messages to be updated or removed based on nick changes,
// departure from channels, and other similar events.
// It is not meant to provide a means of sending responses to events;
// consider using [`Handler`][crate::client::Handler]s for that.
pub trait Adjuster: Send {
    /// Returns `true` if the queue should be adjusted based on this message.
    #[allow(unused_variables)]
    fn should_adjust(&mut self, msg: &ServerMsg<'_>) -> bool {
        true
    }
    /// Updates a single message, returning `false` if it should be removed from the queue.
    fn update(&mut self, msg: &mut ClientMsg<'_>) -> bool;

    /// Resets the adjuster's state (not configuration) to default.
    fn reset(&mut self);
}

/// A collection of multiple arbitrarily-typed [`Adjuster`]s
/// that can be used as a single `Adjuster`.
///
/// The `should_adjust` implementation calls every member's `should_adjust`,
/// and returns `true` if any of them return `true`.
/// The `update` implementation calls `update` for every member that returned `true`
/// during the previous `should_adjust` call, and returns `false` if any of them return `false`.
#[derive(Default)]
pub struct MultiAdjuster {
    adjusters: Vec<(Box<dyn Adjuster>, bool)>,
}

impl MultiAdjuster {
    /// Creates a new, empty `MultiAdjuster`.
    pub fn new() -> MultiAdjuster {
        MultiAdjuster { adjusters: Vec::new() }
    }
    /// Adds an adjuster to the collection.
    ///
    /// Added adjusters will not update messages until at least the next call of `should_adjust`.
    pub fn add<T: Adjuster + 'static>(&mut self, adjuster: T) {
        self.adjusters.push((Box::new(adjuster), false));
    }
    /// Removes all adjusters from `self`.
    pub fn clear(&mut self) {
        self.adjusters.clear();
    }
    /// Returns true if there are no adjusters.
    pub fn is_empty(&self) -> bool {
        self.adjusters.is_empty()
    }
    /// Returns the number of adjusters contained by `self`.
    pub fn len(&self) -> usize {
        self.adjusters.len()
    }
}

impl FromIterator<Box<dyn Adjuster>> for MultiAdjuster {
    fn from_iter<I: IntoIterator<Item = Box<dyn Adjuster>>>(iter: I) -> Self {
        MultiAdjuster { adjusters: iter.into_iter().map(|b| (b, false)).collect() }
    }
}

impl Adjuster for MultiAdjuster {
    fn should_adjust(&mut self, msg: &ServerMsg<'_>) -> bool {
        let mut retval = false;
        for (adj, should_adjust) in &mut self.adjusters {
            *should_adjust = adj.should_adjust(msg);
            retval |= *should_adjust;
        }
        retval
    }
    fn update(&mut self, msg: &mut ClientMsg<'_>) -> bool {
        let mut retval = true;
        for (adj, should_adjust) in &mut self.adjusters {
            if *should_adjust {
                retval &= adj.update(msg);
            }
        }
        retval
    }
    fn reset(&mut self) {
        for (adj, should_adjust) in &mut self.adjusters {
            adj.reset();
            *should_adjust = false;
        }
    }
}
