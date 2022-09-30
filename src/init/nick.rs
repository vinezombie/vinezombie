//! Nickname generation and fallback strategies.

use crate::IrcWord;
use std::error::Error;

/// Error indicating that a nickname generator cannot generate any more nicknames.
#[derive(Clone, Copy, Default, Debug)]
pub struct EndOfNicks;

impl std::fmt::Display for EndOfNicks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "no more nicknames")
    }
}

impl Error for EndOfNicks {}

/// Types that can be used as nickname generators.
pub trait NickGen {
    /// The type of the fallback iterator.
    type Iter: Iterator<Item = IrcWord<'static>>;
    /// Creates both the first nickname and an iterator of fallback nicknames.
    fn nick_gen(self) -> Option<(IrcWord<'static>, Self::Iter)>;
}

impl<T: IntoIterator<Item = IrcWord<'static>>> NickGen for T {
    type Iter = T::IntoIter;

    fn nick_gen(self) -> Option<(IrcWord<'static>, Self::Iter)> {
        let mut iter = self.into_iter();
        let first = iter.next()?;
        Some((first, iter))
    }
}

impl NickGen for IrcWord<'static> {
    type Iter = std::iter::Empty<IrcWord<'static>>;

    fn nick_gen(self) -> Option<(IrcWord<'static>, Self::Iter)> {
        Some((self, std::iter::empty()))
    }
}

// TODO: More nick generators!
// TODO: Trait for nick generators that can be seeded with a starting nick!

/// Suffixes a nick with several low-quality pesudorandom digits.
///
/// This nick generator uses a 32-bit
/// [LCG](https://en.wikipedia.org/wiki/Linear_congruential_generator).
/// It uses the 3 most significant bits of each run to generate octal digits,
/// which it appends to the provided nick.
#[derive(Clone, Debug)]
pub struct SuffixRandom {
    nick: String,
    seed: u32,
    digits: u8,
}

impl SuffixRandom {
    /// Creates a new nick generator seeded from the current time.
    ///
    /// The seed is derived from the current UNIX timestamp at high resolutions where possible.
    pub fn new<'a>(nick: impl Into<IrcWord<'a>>, digits: u8) -> SuffixRandom {
        use std::time;
        let seed = if let Ok(dur) = time::SystemTime::now().duration_since(time::UNIX_EPOCH) {
            (dur.as_millis() as u32) ^ (dur.as_nanos() as u32)
        } else {
            // Something's wrong with the wall clock.
            // Hope the current stack frame's address is random enough instead.
            std::ptr::addr_of!(digits) as u32
        };
        Self::with_seed(nick, digits, seed)
    }
    /// Creates a new nick generator with the specified seed.
    pub fn with_seed<'a>(nick: impl Into<IrcWord<'a>>, digits: u8, seed: u32) -> SuffixRandom {
        let digits_usize: usize = digits.into();
        let prefix = nick.into();
        let mut nick = String::with_capacity(prefix.len_bytes() + digits_usize);
        nick.push_str(prefix.as_ref());
        nick.extend(std::iter::repeat('0').take(digits_usize));
        let mut retval = SuffixRandom { nick, seed, digits };
        retval.step();
        retval
    }
    fn lcg(int: u32) -> u32 {
        // LCG constants taken from Numerical Recipes.
        int.wrapping_mul(1664525).wrapping_add(1013904223)
    }
    fn step(&mut self) {
        let iter = unsafe { self.nick.as_bytes_mut().iter_mut().rev().take(self.digits.into()) };
        for digit in iter {
            self.seed = SuffixRandom::lcg(self.seed);
            *digit = b'0' + (self.seed >> 29) as u8;
        }
    }
}

impl Iterator for SuffixRandom {
    type Item = IrcWord<'static>;

    fn next(&mut self) -> Option<Self::Item> {
        let retval = unsafe { IrcWord::new_unchecked(self.nick.as_str()).owning() };
        self.step();
        Some(retval)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    pub fn gen_crude() {
        use super::{NickGen, SuffixRandom};
        let gen = SuffixRandom::with_seed("Foo", 9, 1337);
        let mut prev: u32 = 9;
        let (mut nick, mut rest) = gen.nick_gen().unwrap();
        for _ in 0..16 {
            let num = nick.strip_prefix("Foo").unwrap();
            assert_eq!(num.len(), 9);
            let num: u32 = num.parse().unwrap();
            assert_ne!(num, prev);
            prev = num;
            nick = rest.next().unwrap();
        }
    }
}
