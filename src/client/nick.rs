//! Nickname generation and fallback strategies.

mod suffix;
#[cfg(test)]
mod tests;

pub use suffix::*;

use crate::string::{Builder, Nick};
use std::{error::Error, iter::FusedIterator};

type NickBuilder = Builder<Nick<'static>>;

/// Standard nickname options.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde_derive::Serialize, serde_derive::Deserialize))]
pub struct Nicks<N> {
    /// The list of nicknames to use.
    pub nicks: Vec<Nick<'static>>,
    /// Whether to skip attempting to use the first nickname,
    /// using it only for fallbacks.
    #[cfg_attr(feature = "serde", serde(default))]
    pub skip_first: bool,
    /// The [`NickTransformer`] for generating new nicknames from the first one.
    pub gen: std::sync::Arc<N>,
}

/// Error indicating that a nickname generator cannot generate any more nicknames.
#[derive(Clone, Copy, Default, Debug)]
pub struct EndOfNicks;

impl std::fmt::Display for EndOfNicks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "no more nicknames")
    }
}

impl Error for EndOfNicks {}

/// Nick generators.
///
/// Nick generators always yield at least one nick, are peekable, and have explicit continuations.
pub trait NickGen: 'static {
    /// Generates a new nickname and an optional continuation.
    ///
    /// The returned nick should not depend on the value of `prev_was_invalid`,
    /// which returns `true` if the previous nick generated by
    fn next_nick(self: Box<Self>) -> (Nick<'static>, Option<Box<dyn NickGen>>);
    /// Generates the next nickname without advancing state.
    fn peek(&self) -> Nick<'static>;
    /// Updates the state when a server specifies that a nick is invalid.
    fn handle_invalid(self: Box<Self>, nick: &Nick<'static>) -> Option<Box<dyn NickGen>>;
}

impl<G: NickGen + ?Sized> NickGen for Box<G> {
    fn next_nick(self: Box<Self>) -> (Nick<'static>, Option<Box<dyn NickGen>>) {
        G::next_nick(*self)
    }

    fn peek(&self) -> Nick<'static> {
        G::peek(self)
    }

    fn handle_invalid(self: Box<Self>, nick: &Nick<'static>) -> Option<Box<dyn NickGen>> {
        G::handle_invalid(*self, nick)
    }
}

impl NickGen for Nick<'static> {
    fn next_nick(self: Box<Self>) -> (Nick<'static>, Option<Box<dyn NickGen>>) {
        (*self, None)
    }

    fn peek(&self) -> Nick<'static> {
        self.clone()
    }

    fn handle_invalid(self: Box<Self>, nick: &Nick<'static>) -> Option<Box<dyn NickGen>> {
        let eq = *nick == *self;
        eq.then_some(self)
    }
}

/// An [`Iterator`] being used as a nick generator.
#[derive(Clone, Debug)]
pub struct FromIter<I> {
    nick: Nick<'static>,
    iter: I,
}

/// Wraps an iterator for use as a [`NickGen`].
pub fn from_iter<I: IntoIterator<Item = Nick<'static>>>(iter: I) -> Option<FromIter<I::IntoIter>> {
    let mut iter = iter.into_iter();
    let nick = iter.next()?;
    Some(FromIter { nick, iter })
}

/// Combines an iterator and fallback nickname for use as a [`NickGen`].
pub fn from_iter_or<It: FusedIterator<Item = Nick<'static>>, I: IntoIterator<IntoIter = It>>(
    default: Nick<'static>,
    iter: I,
) -> FromIter<It> {
    let mut iter = iter.into_iter();
    let nick = iter.next().unwrap_or(default);
    FromIter { nick, iter }
}

impl<I: Iterator<Item = Nick<'static>> + 'static> NickGen for FromIter<I> {
    fn next_nick(mut self: Box<Self>) -> (Nick<'static>, Option<Box<dyn NickGen>>) {
        if let Some(mut next) = self.iter.next() {
            std::mem::swap(&mut next, &mut self.nick);
            (next, Some(self))
        } else {
            (self.nick, None)
        }
    }

    fn peek(&self) -> Nick<'static> {
        self.nick.clone()
    }

    fn handle_invalid(self: Box<Self>, _nick: &Nick<'static>) -> Option<Box<dyn NickGen>> {
        let is_finite = self.iter.size_hint().1.is_some();
        is_finite.then_some(self)
    }
}

/// Extension methods for boxed [`NickGen`]s.
pub trait NickGenExt: NickGen + Sized {
    /// Appends the output from the provided nick generator to self.
    fn chain(self: Box<Self>, ng: Box<dyn NickGen>) -> Box<Chain> {
        Box::new(Chain { a: Some(self), b: Some(ng) })
    }
    /// As [`NickGenExt::chain()`] using the next nick from `self` and a [`NickTransformer`].
    /// Does not advance `self`.
    fn chain_using(self: Box<Self>, nt: &impl NickTransformer) -> Box<Chain> {
        let b = nt.transform(self.peek());
        self.chain(b)
    }
}

impl<T: NickGen> NickGenExt for T {}

/// Two [`NickGen`]s in sequence.
pub struct Chain {
    pub(self) a: Option<Box<dyn NickGen>>,
    /// Postcondition of every Chain method: b must not be None.
    pub(self) b: Option<Box<dyn NickGen>>,
}

impl NickGen for Chain {
    fn next_nick(mut self: Box<Self>) -> (Nick<'static>, Option<Box<dyn NickGen>>) {
        if let Some(gen) = self.a.take() {
            let (nick, next) = gen.next_nick();
            self.a = next;
            (nick, Some(self))
        } else {
            let (nick, next) = self.b.unwrap().next_nick();
            (nick, next)
        }
    }

    fn peek(&self) -> Nick<'static> {
        let next = self.a.as_deref().unwrap_or(self.b.as_deref().unwrap());
        next.peek()
    }

    fn handle_invalid(mut self: Box<Self>, nick: &Nick<'static>) -> Option<Box<dyn NickGen>> {
        let Some(a) = self.a.take().and_then(|a| a.handle_invalid(nick)) else {
            return self.b.unwrap().handle_invalid(nick);
        };
        let Some(b) = self.b.take().unwrap().handle_invalid(nick) else {
            return Some(a);
        };
        self.a = Some(a);
        self.b = Some(b);
        Some(self)
    }
}

/// Types that can be used to produce nick generators from existing nicks.
///
/// Implementations can often be deserialized,
/// but there is also a blanket implementation for appropriately-typed functions.
pub trait NickTransformer {
    /// Transforms a nick into a nick generator.
    fn transform(&self, nick: Nick<'static>) -> Box<dyn NickGen>;
}
