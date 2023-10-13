use std::{
    collections::BinaryHeap,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug)]
struct DelayPair<T>(Instant, T);

impl<T> PartialEq for DelayPair<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<T> PartialOrd for DelayPair<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        // Use the Ord impl.
        Some(self.cmp(other))
    }
}

impl<T> Eq for DelayPair<T> {}

impl<T> Ord for DelayPair<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0).reverse()
    }
}

/// A collection of delayed values used by sychronous I/O handlers.
///
/// This queue is used to specify read timeouts and pass values back to the handler
/// when those timeouts happen.
#[derive(Clone, Debug, Default)]
pub struct DelayQueue<T> {
    queue: BinaryHeap<DelayPair<T>>,
    default: Option<Duration>,
}

impl<T> DelayQueue<T> {
    /// Creates a new, empty queue with an indefinite default timeout.
    pub fn new() -> Self {
        DelayQueue { queue: BinaryHeap::new(), default: None }
    }
    /// Adds an item to the queue along with a time after which it should be returned.
    pub fn push(&mut self, value: T, return_after: Instant) {
        self.queue.push(DelayPair(return_after, value));
    }
    /// Returns the number if items in the queue.
    pub fn len(&self) -> usize {
        self.queue.len()
    }
    /// Returns `true` if there are no items in the queue.
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
    /// Unconditionally returns the next item in the queue, removing it.
    ///
    /// This function returns the next item in the queue without regard for
    /// whether its `return_after` value is in the past or not.
    /// See [`next_timeout`][DelayQueue::next_timeout].
    pub fn pop(&mut self) -> Option<T> {
        self.queue.pop().map(|v| v.1)
    }
    /// Views the next item in the queue.
    pub fn peek(&self) -> Option<&T> {
        self.queue.peek().map(|dp| &dp.1)
    }
    /// Sets the default timeout to be used when the queue is empty.
    ///
    /// It is usually a logic error to set this duration to zero.
    pub fn set_default_timeout(&mut self, timeout: Option<Duration>) -> &mut Self {
        self.default = timeout;
        self
    }
    /// Returns how long the next read timeout should last,
    /// or `None` for "indefinitely".
    ///
    /// This function will frequently return a duration of zero,
    /// which should be checked-for by the caller as it's usually an invalid timeout duration.
    /// This should be taken to mean that a value is available to remove using [`pop`][DelayQueue::pop].
    pub fn next_timeout(&self) -> Option<Duration> {
        if let Some(next) = self.queue.peek() {
            Some(next.0.saturating_duration_since(Instant::now()))
        } else {
            self.default
        }
    }
}
