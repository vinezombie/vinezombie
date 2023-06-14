//! Nickname generation and fallback strategies.

use crate::string::Nick;
use std::{borrow::Cow, error::Error};

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
    fn step(&self, state: Self::State) -> (Nick<'static>, Option<Self::State>);
}

impl NickTransformer for () {
    type State = std::convert::Infallible;

    fn init(&self, _: &Nick<'static>) -> Option<(Nick<'static>, Option<Self::State>)> {
        None
    }

    fn step(&self, state: Self::State) -> (Nick<'static>, Option<Self::State>) {
        match state {}
    }
}

/// Possible randomized suffixes that can be added to a nickname.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum NickSuffix {
    /// One ASCII letter, uppercase if `true`.
    Letter(bool),
    /// One octal digit.
    Base8,
    /// One decimal digit.
    Base10,
    /// One of the strings in the provided [`Vec`].
    ///
    /// Only the first 256 elements will be considered.
    Choice(Vec<Nick<'static>>),
}

impl NickSuffix {
    fn append(&self, num: u8, nick: &mut Vec<u8>) {
        match self {
            NickSuffix::Letter(true) => nick.push(b'A' + num % 26),
            NickSuffix::Letter(false) => nick.push(b'a' + num % 26),
            NickSuffix::Base8 => nick.push(b'0' + (num & 7)),
            NickSuffix::Base10 => nick.push(b'0' + num % 10),
            NickSuffix::Choice(opts) if !opts.is_empty() => {
                let idx = num as usize % opts.len();
                nick.extend_from_slice(opts[idx].as_bytes());
            }
            _ => (),
        }
    }
}

/// Suffixes a nick with several pseudorandomly-chosen suffixes.
///
/// This nick generator uses a 32-bit
/// [LCG](https://en.wikipedia.org/wiki/Linear_congruential_generator).
/// It appends one value from each element in `suffixes`.
#[derive(Clone, PartialEq, Eq, Hash, Default, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SuffixRandom {
    /// The seed to use for nick generation, or `None` for a pseudo-random one.
    pub seed: Option<u32>,
    /// What to suffix onto the nick.
    pub suffixes: Cow<'static, [NickSuffix]>,
}

impl SuffixRandom {
    fn gen(&self, prefix: &Nick<'static>, seed: &mut u32) -> Nick<'static> {
        // TODO: Incorrect preallocation when one of the NickSuffixes is Choice.
        let mut retval = Vec::with_capacity(prefix.len() + self.suffixes.len());
        retval.extend_from_slice(prefix.as_bytes());
        let mut cycle = true;
        for suffix in self.suffixes.as_ref() {
            if cycle {
                // LCG constants taken from Numerical Recipes.
                *seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                suffix.append((*seed >> 16) as u8, &mut retval);
                cycle = false;
            } else {
                suffix.append((*seed >> 24) as u8, &mut retval);
                cycle = true;
            }
        }
        unsafe { Nick::from_unchecked(retval.into()) }
    }
}

impl NickTransformer for SuffixRandom {
    type State = (Nick<'static>, u32);

    fn init(&self, prefix: &Nick<'static>) -> Option<(Nick<'static>, Option<Self::State>)> {
        use std::time::{SystemTime, UNIX_EPOCH};
        let mut seed = if let Some(seed) = self.seed {
            seed
        } else {
            // TODO: Hash prefix using RandomState (awaiting Rust 1.71)
            let mut seed = 0u32;
            if let Ok(dur) = SystemTime::now().duration_since(UNIX_EPOCH) {
                seed ^= dur.as_millis() as u32;
                seed ^= dur.as_nanos() as u32;
            }
            seed
        };
        let nick = self.gen(prefix, &mut seed);
        Some((nick, Some((prefix.clone(), seed))))
    }

    fn step(&self, mut state: Self::State) -> (Nick<'static>, Option<Self::State>) {
        let nick = self.gen(&state.0, &mut state.1);
        (nick, Some(state))
    }
}

#[cfg(test)]
mod tests {
    use crate::string::Nick;

    #[test]
    pub fn gen_crude() {
        use super::{NickSuffix, NickTransformer, SuffixRandom};
        let prefix = Nick::from_bytes("Foo").unwrap();
        let gen = SuffixRandom { suffixes: vec![NickSuffix::Base8; 9].into(), seed: Some(1337) };
        let (mut nick, mut state) = gen.init(&prefix).unwrap();
        let mut prev: u32 = 9;
        for _ in 0..16 {
            let nick_str = nick.to_utf8().unwrap();
            let num = nick_str.strip_prefix("Foo").unwrap();
            assert_eq!(num.len(), 9);
            let num: u32 = num.parse().unwrap();
            assert_ne!(num, prev);
            prev = num;
            (nick, state) = gen.step(state.unwrap());
        }
    }
}
