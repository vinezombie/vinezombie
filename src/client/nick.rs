//! Nickname generation and fallback strategies.

use crate::string::{Nick, NickBuilder};
use std::{borrow::Cow, error::Error};

/// Standard nickname options.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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

/// Utilities for generating nicknames after the nick list runs out.
///
/// Nick transformers have external state,
/// allowing their configuration to be more-easily serialized/deserialized.
pub trait NickTransformer {
    /// The transformer state.
    type State: Sized;
    /// Creates the initial nickname and transformer state.
    fn init(&self, nick: &Nick<'static>) -> Option<(Nick<'static>, Option<Self::State>)>;
    /// Returns the next nickname from the provided transformer state.
    fn step(state: Self::State) -> (Nick<'static>, Option<Self::State>);
}

impl NickTransformer for () {
    type State = std::convert::Infallible;

    fn init(&self, _: &Nick<'static>) -> Option<(Nick<'static>, Option<Self::State>)> {
        None
    }

    fn step(state: Self::State) -> (Nick<'static>, Option<Self::State>) {
        match state {}
    }
}

impl<I: Iterator<Item = Nick<'static>>, F: Fn(&Nick<'static>) -> I> NickTransformer for F {
    type State = (Nick<'static>, I);

    fn init(&self, nick: &Nick<'static>) -> Option<(Nick<'static>, Option<Self::State>)> {
        let mut iter = self(nick);
        let nick = iter.next()?;
        Some(Self::step((nick, iter)))
    }

    fn step(state: Self::State) -> (Nick<'static>, Option<Self::State>) {
        let (nick, mut iter) = state;
        if let Some(next_nick) = iter.next() {
            (nick, Some((next_nick, iter)))
        } else {
            (nick, None)
        }
    }
}

/// Possible randomized suffixes that can be added to a nickname.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum SuffixType {
    /// One ASCII letter, uppercase if `true`.
    Letter(bool),
    /// One octal digit.
    Base8,
    /// One decimal digit.
    Base10,
    /// One hexadecimal digit, uppercase if `true`.
    Base16(bool),
    /// One nonzero octal digit.
    NonZeroBase8,
    /// One nonzero decimal digit.
    NonZeroBase10,
    /// One nonzero hexadecimal digit, uppercase if `true`.
    NonZeroBase16(bool),
    /// One of the strings in the provided [`Vec`].
    ///
    /// Only the first 256 elements will be considered.
    Choice(Vec<Nick<'static>>),
    /// One character.
    Char(char),
}

impl SuffixType {
    fn append(&self, num: u8, nick: &mut NickBuilder) {
        let _ = match self {
            SuffixType::Letter(true) => nick.try_push(b'A' + num % 26),
            SuffixType::Letter(false) => nick.try_push(b'a' + num % 26),
            SuffixType::Base8 => nick.try_push(b'0' + (num & 7)),
            SuffixType::Base10 => nick.try_push(b'0' + num % 10),
            SuffixType::Base16(uc) => nick.try_push(match num & 15 {
                num if *uc && num > 9 => b'A' + num,
                num if !uc && num > 9 => b'a' + num,
                num => b'0' + num,
            }),
            SuffixType::NonZeroBase8 => nick.try_push(b'1' + (num % 7)),
            SuffixType::NonZeroBase10 => nick.try_push(b'1' + num % 9),
            SuffixType::NonZeroBase16(uc) => nick.try_push(match num % 15 {
                num if *uc && num > 8 => b'A' + num,
                num if !uc && num > 8 => b'a' + num,
                num => b'1' + num,
            }),
            SuffixType::Choice(opts) if !opts.is_empty() => {
                let idx = num as usize % opts.len();
                nick.append(opts[idx].clone());
                Ok(())
            }
            SuffixType::Choice(_) => Ok(()),
            SuffixType::Char(c) => nick.try_push_char(*c),
        };
    }
    fn variance(&self) -> u8 {
        match self {
            SuffixType::Letter(_) => 26,
            SuffixType::Base8 => 8,
            SuffixType::Base10 => 10,
            SuffixType::Base16(_) => 16,
            SuffixType::NonZeroBase8 => 7,
            SuffixType::NonZeroBase10 => 9,
            SuffixType::NonZeroBase16(_) => 15,
            SuffixType::Choice(c) => std::cmp::min(255, c.len()) as u8,
            SuffixType::Char(_) => 1,
        }
    }
}

/// Suffixes a nick with several pseudorandomly-chosen suffixes.
#[derive(Clone, PartialEq, Eq, Hash, Default, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Suffix {
    /// See [`SuffixStrategy`].
    pub strategy: SuffixStrategy,
    /// What to suffix onto the nick.
    pub suffixes: Cow<'static, [SuffixType]>,
}

/// The method by which suffixes should be selected for addition to a nickname.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum SuffixStrategy {
    /// Pseudorandomly choose nick suffixes, optionally using the provided seed.
    ///
    /// This uses a 32-bit
    /// [LCG](https://en.wikipedia.org/wiki/Linear_congruential_generator).
    /// It appends one value from each element in `suffixes`.
    ///
    /// The exact nicknames returned by this strategy should NOT be relied upon,
    /// and may change with minor version bumps.
    Rng(Option<u32>),
    /// Iterate through values of the first suffix, then the first two, and so on.
    ///
    /// If you want to reimplement the common behavior of suffixing a nick with an
    /// increasing number of underscores, this is what you want to use.
    Seq,
}

impl Default for SuffixStrategy {
    fn default() -> Self {
        SuffixStrategy::Rng(None)
    }
}

/// Opaque state for [`Suffix`].
#[derive(Clone, Debug)]
pub struct SuffixState {
    cfg: Suffix,
    prefix: Nick<'static>,
    state: u32,
    limit: u8,
}

impl NickTransformer for Suffix {
    type State = SuffixState;

    fn init(&self, prefix: &Nick<'static>) -> Option<(Nick<'static>, Option<Self::State>)> {
        use std::time::{SystemTime, UNIX_EPOCH};
        if self.suffixes.is_empty() {
            return None;
        }
        let (seed, limit) = match self.strategy {
            SuffixStrategy::Rng(seed) => {
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{BuildHasher, BuildHasherDefault};
                let limit = self
                    .suffixes
                    .iter()
                    .fold(1u8, |count, suffix| count.saturating_add(suffix.variance() / 2));
                if let Some(seed) = seed {
                    (seed, limit)
                } else {
                    let hasher = BuildHasherDefault::<DefaultHasher>::default();
                    let mut seed = hasher.hash_one(prefix) as u32;
                    if let Ok(dur) = SystemTime::now().duration_since(UNIX_EPOCH) {
                        seed ^= dur.as_millis() as u32;
                        seed ^= dur.as_nanos() as u32;
                    }
                    (seed, limit)
                }
            }
            SuffixStrategy::Seq => {
                let limit = self
                    .suffixes
                    .iter()
                    .fold(0u8, |count, suffix| count.saturating_add(suffix.variance()));
                (0, limit)
            }
        };
        let state = SuffixState { cfg: self.clone(), prefix: prefix.clone(), state: seed, limit };
        Some(Self::step(state))
    }

    fn step(mut state: Self::State) -> (Nick<'static>, Option<Self::State>) {
        let mut nick = NickBuilder::new_from(state.prefix.clone());
        match state.cfg.strategy {
            SuffixStrategy::Rng(_) => {
                nick.reserve(state.cfg.suffixes.len());
                let mut cycle = true;
                for suffix in state.cfg.suffixes.iter() {
                    if cycle {
                        // LCG constants taken from Numerical Recipes.
                        state.state = state.state.wrapping_mul(1664525).wrapping_add(1013904223);
                        suffix.append((state.state >> 16) as u8, &mut nick);
                        cycle = false;
                    } else {
                        suffix.append((state.state >> 24) as u8, &mut nick);
                        cycle = true;
                    }
                }
            }
            SuffixStrategy::Seq => {
                state.state += 1;
                nick.reserve(std::cmp::min(state.cfg.suffixes.len(), state.state as usize));
                let mut count = state.state;
                for suffix in state.cfg.suffixes.iter() {
                    if count == 0 {
                        break;
                    }
                    let variance = suffix.variance();
                    if variance == 0 {
                        continue;
                    }
                    count -= 1;
                    suffix.append(count as u8 % variance, &mut nick);
                    count /= variance as u32;
                }
            }
        };
        state.limit -= 1;
        (nick.build(), (state.limit != 0).then_some(state))
    }
}

#[cfg(test)]
mod tests {
    use super::{NickTransformer, Suffix, SuffixStrategy, SuffixType};
    use crate::string::Nick;

    #[test]
    pub fn suffix_rng() {
        let prefix = Nick::from_bytes("Foo").unwrap();
        let gen = Suffix {
            suffixes: vec![SuffixType::Base8; 9].into(),
            strategy: SuffixStrategy::Rng(Some(1337)),
        };
        let (mut nick, mut state) = gen.init(&prefix).unwrap();
        let mut prev: u32 = 9;
        for _ in 0..16 {
            let nick_str = nick.to_utf8().unwrap();
            let num = nick_str.strip_prefix("Foo").unwrap();
            assert_eq!(num.len(), 9);
            let num: u32 = num.parse().unwrap();
            assert_ne!(num, prev);
            prev = num;
            (nick, state) = Suffix::step(state.unwrap());
        }
    }

    #[test]
    pub fn suffix_seq() {
        let prefix = Nick::from_bytes("Foo").unwrap();
        let gen = Suffix {
            suffixes: vec![
                SuffixType::Char('_'),
                SuffixType::Char('_'),
                SuffixType::NonZeroBase8,
                SuffixType::Base8,
            ]
            .into(),
            strategy: SuffixStrategy::Seq,
        };
        let (mut nick, mut state) = gen.init(&prefix).unwrap();
        assert_eq!(nick, "Foo_");
        (nick, state) = Suffix::step(state.unwrap());
        assert_eq!(nick, "Foo__");
        (nick, state) = Suffix::step(state.unwrap());
        assert_eq!(nick, "Foo__1");
        for _ in 1..7 {
            (nick, state) = Suffix::step(state.unwrap());
        }
        assert_eq!(nick, "Foo__7");
        (nick, state) = Suffix::step(state.unwrap());
        assert_eq!(nick, "Foo__10");
        (nick, _) = Suffix::step(state.unwrap());
        assert_eq!(nick, "Foo__20");
    }
}
