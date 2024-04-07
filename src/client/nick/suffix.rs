use super::{NickBuilder, NickGen, NickTransformer};
use crate::string::Nick;
use std::borrow::Cow;

/// Possible randomized suffixes that can be added to a nickname.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde_derive::Serialize, serde_derive::Deserialize))]
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

/// Suffixes the first yielded nick with several pseudorandomly-chosen suffixes.
#[derive(Clone, PartialEq, Eq, Hash, Default, Debug)]
#[cfg_attr(feature = "serde", derive(serde_derive::Serialize, serde_derive::Deserialize))]
pub struct Suffix {
    /// See [`SuffixStrategy`].
    pub strategy: SuffixStrategy,
    /// What to suffix onto the nick.
    pub suffixes: Cow<'static, [SuffixType]>,
}

/// The method by which suffixes should be selected for addition to a nickname.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde_derive::Serialize, serde_derive::Deserialize))]
#[non_exhaustive]
pub enum SuffixStrategy {
    /// Pseudorandomly choose nick suffixes, optionally using the provided seed.
    /// If no seed is specified, a random seed is chosen.
    ///
    /// This uses a 32-bit
    /// [LCG](https://en.wikipedia.org/wiki/Linear_congruential_generator).
    /// It appends one value from each element in `suffixes`.
    ///
    /// The exact nicknames returned by this strategy should NOT be relied upon,
    /// and may even change between patch versions.
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

/// Nick generator yielded by [`Suffix`].
#[derive(Clone, Debug)]
pub struct SuffixGen {
    cfg: Suffix,
    prefix: Nick<'static>,
    state: u32,
    limit: u8,
}

impl NickTransformer for Suffix {
    fn transform(&self, prefix: Nick<'static>) -> Box<dyn NickGen> {
        use std::time::{SystemTime, UNIX_EPOCH};
        if self.suffixes.is_empty() {
            return Box::new(SuffixGen { cfg: Suffix::default(), prefix, state: 0, limit: 1 });
        }
        let (seed, limit) = match self.strategy {
            SuffixStrategy::Rng(seed) => {
                let limit = self
                    .suffixes
                    .iter()
                    .fold(1u8, |count, suffix| count.saturating_add(suffix.variance() / 2));
                if let Some(seed) = seed {
                    (seed, limit)
                } else {
                    let mut seed = crate::util::mangle(&prefix);
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
        Box::new(SuffixGen { cfg: self.clone(), prefix, state: seed, limit })
    }
}

impl SuffixGen {
    fn calc_next(&self) -> (Nick<'static>, u32) {
        let mut nick = NickBuilder::new(self.prefix.clone());
        let mut state = self.state;
        match self.cfg.strategy {
            SuffixStrategy::Rng(_) => {
                nick.reserve(self.cfg.suffixes.len());
                let mut cycle = true;
                for suffix in self.cfg.suffixes.iter() {
                    if cycle {
                        // LCG constants taken from Numerical Recipes.
                        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
                        suffix.append((state >> 16) as u8, &mut nick);
                        cycle = false;
                    } else {
                        suffix.append((state >> 24) as u8, &mut nick);
                        cycle = true;
                    }
                }
            }
            SuffixStrategy::Seq => {
                state += 1;
                nick.reserve(std::cmp::min(self.cfg.suffixes.len(), state as usize));
                let mut count = state;
                for suffix in self.cfg.suffixes.iter() {
                    let variance = suffix.variance();
                    if variance == 0 {
                        continue;
                    }
                    count -= 1;
                    suffix.append(count as u8 % variance, &mut nick);
                    count /= variance as u32;
                    if count == 0 {
                        break;
                    }
                }
            }
        };
        (nick.build(), state)
    }
}

impl NickGen for SuffixGen {
    fn next_nick(mut self: Box<Self>) -> (Nick<'static>, Option<Box<dyn NickGen>>) {
        let (nick, state) = self.calc_next();
        self.state = state;
        self.limit -= 1;
        let not_done = self.limit != 0;
        (nick, not_done.then_some(self))
    }

    fn peek(&self) -> Nick<'static> {
        self.calc_next().0
    }

    fn handle_invalid(self: Box<Self>, nick: &Nick<'static>) -> Option<Box<dyn NickGen>> {
        // Somewhat over-aggressive, but likely good enough.
        if nick.starts_with(&self.prefix) {
            None
        } else {
            Some(self)
        }
    }
}
