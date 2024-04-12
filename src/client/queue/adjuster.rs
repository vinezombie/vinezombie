//! The [`Adjuster`] trait and useful implementations of it.
//!
//! This exists to allow queued messages to be updated or removed based on nick changes,
//! departure from channels, and other similar events.
//! It is not meant to provide a means of sending responses to events;
//! consider using [`Handler`][crate::client::Handler]s for that.

use crate::ircmsg::{ClientMsg, ServerMsg};

/// Logic for adjusting a [`Queue`][super::Queue] based on incoming messages.
///
/// See the [module-level documentation][self] for more.
pub trait Adjuster {
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

impl Adjuster for () {
    fn should_adjust(&mut self, _: &ServerMsg<'_>) -> bool {
        false
    }
    fn update(&mut self, _: &mut ClientMsg<'_>) -> bool {
        true
    }
    fn reset(&mut self) {}
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
