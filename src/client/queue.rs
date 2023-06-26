use crate::ircmsg::ClientMsg;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// A rate-limited queue for client messages.
///
/// RFC 1459 recommends sending messages as a client in bursts of up to 5 messages,
/// followed by one message every 2 seconds.
/// This type enforces that recommendation by resticting how frequently
/// messages can be removed from it.
#[derive(Clone, Debug)]
pub struct Queue<'a> {
    queue: VecDeque<ClientMsg<'a>>,
    delay: Duration,
    sub: Duration,
    timepoint: Instant,
}

impl<'a> Default for Queue<'a> {
    fn default() -> Self {
        Self::new()
    }
}
impl<'a> FromIterator<ClientMsg<'a>> for Queue<'a> {
    fn from_iter<T: IntoIterator<Item = ClientMsg<'a>>>(iter: T) -> Self {
        Self::from_queue(iter.into_iter().collect())
    }
}
impl<'a> From<Vec<ClientMsg<'a>>> for Queue<'a> {
    fn from(value: Vec<ClientMsg<'a>>) -> Self {
        Self::from_queue(value.into())
    }
}
impl<'a> From<VecDeque<ClientMsg<'a>>> for Queue<'a> {
    fn from(value: VecDeque<ClientMsg<'a>>) -> Self {
        Self::from_queue(value)
    }
}

impl<'a> Queue<'a> {
    /// Creates a new queue with the default rate limit.
    pub fn new() -> Self {
        Self::from_queue(VecDeque::with_capacity(4))
    }
    fn from_queue(queue: VecDeque<ClientMsg<'a>>) -> Self {
        Queue {
            queue,
            delay: Duration::from_secs(2),
            sub: Duration::from_secs(8),
            timepoint: Instant::now(),
        }
    }
    /// Changes the rate limit.
    ///
    /// `delay` specifies how much time should pass between messages.
    /// `burst` specifies how many additional messages may be sent during an initial burst,
    /// e.g. a value of `4` results in a burst of five messages.
    pub fn set_rate_limit(&mut self, delay: Duration, burst: u32) {
        self.delay = delay;
        self.sub = delay.saturating_mul(burst);
        // Pessimistically sets the next-message delay
        // to the longest possible under the new settings.
        let now = Instant::now();
        self.timepoint = now.checked_add(self.sub.saturating_add(delay)).unwrap_or(now);
    }
    /// Add a message onto the end of a queue.
    pub fn push(&mut self, msg: ClientMsg<'a>) {
        self.queue.push_back(msg);
    }
    /// Retrieves a message from the queue, subject to rate limits.
    ///
    /// If this function does not return a message,
    /// `timeout_fn` is called with the duration until the next message will be available,
    /// or `None` if the queue is empty.
    /// The duration is guaranteed to be non-zero. This can be used to adjust read timeouts.
    pub fn pop(&mut self, timeout_fn: impl FnOnce(Option<Duration>)) -> Option<ClientMsg<'a>> {
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
    // TODO: Queue cleaning.
}
