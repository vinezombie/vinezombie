//! IRC message argument utilities.

use crate::{
    error::InvalidString,
    string::{Arg, Line, Splitter},
};
use std::borrow::Cow;

/// IRC message argument list.
///
/// This type enforces the invariant that
/// only the last argument may be longer than one word.
#[derive(Clone, PartialEq, Eq, Hash, Default, Debug)]
pub struct Args<'a> {
    words: Cow<'a, [Arg<'a>]>,
    long: Option<Line<'a>>,
}

/// Guard for editing [`Args`].
#[derive(PartialEq, Eq, Hash, Debug)]
pub struct ArgsEditGuard<'a, 'b>(&'b mut Vec<Arg<'a>>, &'b mut Option<Line<'a>>);

impl<'a> ArgsEditGuard<'a, '_> {
    /// Adds an [`Arg`] to the argument list.
    ///
    /// The argument will be added to the end of the argument list unless the last argument is
    /// long, in which case it will be added as the second-to-last argument.
    pub fn add_word<'b: 'a>(&mut self, w: impl Into<Arg<'b>>) {
        self.0.push(w.into());
    }
    /// Adds a word to the argument list in the second-to-last position.
    ///
    /// If the argument list is empty, just adds the word.
    /// Use this when the last argument in the list has special meaning.
    pub fn add_before_last<'b: 'a>(&mut self, w: impl Into<Arg<'b>>) {
        let mut s = w.into();
        if self.1.is_none() {
            if let Some(last) = self.0.last_mut() {
                std::mem::swap(&mut s, last);
            }
        }
        self.0.push(s);
    }
    /// Adds a string to the end of this argument list.
    ///
    /// If the last string in the argument list is long,
    /// it will be replaced and returned.
    pub fn add<'b: 'a>(&mut self, s: impl Into<Line<'b>>) -> Option<Line<'a>> {
        add_impl(self.0, self.1, s.into())
    }
    /// A panicing version of [`ArgsEditGuard::add`] for string literals.
    ///
    /// # Panics
    /// Panics if the provided string literal is not a valid [`Line`],
    /// that is, if it contains an `'\r'`, `'\n'`, or `'\0'`.
    pub fn add_literal(&mut self, s: &'static str) {
        self.add(Line::from_str(s));
    }
    /// Returns how many more arguments can be safely added to `self`.
    ///
    /// IRC messages should contain no more than 15 arguments,
    /// but IRC software should also support any number of arguments.
    pub fn args_left(&self) -> isize {
        15 - self.0.len() as isize
    }
    /// Returns a mutable slice of all the arguments as a slice of [`Arg`]s,
    /// or None if the final argument is long.
    pub fn all(&mut self) -> Option<&mut [Arg<'a>]> {
        self.1.is_none().then_some(self.0.as_mut_slice())
    }
    /// Returns a mutable slice of all the non-long aruments.
    pub fn words(&mut self) -> &mut [Arg<'a>] {
        self.0
    }
    /// Returns a mutable slice of arguments with the last argument split off.
    pub fn split_last(&mut self) -> (&mut [Arg<'a>], Option<&Line<'a>>) {
        if let Some(long) = &mut self.1 {
            (self.0.as_mut_slice(), Some(long))
        } else if let Some((last, rest)) = self.0.split_last_mut() {
            (rest, Some(last))
        } else {
            (&mut [], None)
        }
    }
    /// Clears the argument list.
    ///
    /// This operation preserves memory already allocated for the argument list.
    /// If you want to avoid this, use [`args.clear()`][Args::clear].
    pub fn clear(&mut self) {
        self.0.clear();
        *self.1 = None;
    }
}

fn add_impl<'a>(
    words: &mut Vec<Arg<'a>>,
    long: &mut Option<Line<'a>>,
    s: Line<'a>,
) -> Option<Line<'a>> {
    let is_long = Arg::find_invalid(&s).is_some();
    if is_long {
        long.replace(s)
    } else {
        // Safety: We just checked this.
        words.push(unsafe { Arg::from_unchecked(s.into_bytes()) });
        long.take()
    }
}

impl<'a> Args<'a> {
    /// Creates a new empty argument list.
    pub const fn empty() -> Args<'a> {
        Args { words: Cow::Owned(Vec::new()), long: None }
    }
    /// Creates a new argument list from the provided arguments.
    pub fn new(words: impl Into<Cow<'a, [Arg<'a>]>>, last: Option<Line<'a>>) -> Args<'a> {
        let mut words = words.into();
        let mut long = None;
        if let Some(last) = last {
            add_impl(words.to_mut(), &mut long, last);
        }
        Args { words, long }
    }
    /// Returns a guard that allows editing of `self`.
    pub fn edit(&mut self) -> ArgsEditGuard<'a, '_> {
        ArgsEditGuard(self.words.to_mut(), &mut self.long)
    }
    /// Converts `self` into a version that owns its data.
    pub fn owning(self) -> Args<'static> {
        #[allow(clippy::unnecessary_to_owned)]
        Args {
            long: self.long.map(Line::owning),
            // rustc optimizations mean that the map operation should run in-place.
            words: Cow::Owned(self.words.into_owned().into_iter().map(Arg::owning).collect()),
        }
    }
    /// Parses an argument list from a string.
    pub fn parse(line: impl Into<Line<'a>>) -> Args<'a> {
        let mut line = Splitter::new(line.into());
        // Many IRC messages have no more than 2 arguments.
        let mut words = Vec::with_capacity(2);
        let mut long = None;
        loop {
            line.consume_whitespace();
            match line.string::<Arg>(false) {
                Ok(arg) => words.push(arg),
                Err(InvalidString::Colon) => {
                    line.next_byte();
                    add_impl(&mut words, &mut long, line.rest_or_default());
                    break;
                }
                Err(_) => break,
            }
        }
        Args { words: Cow::Owned(words), long }
    }
    /// Returns true if there are no arguments.
    pub fn is_empty(&self) -> bool {
        // Why is Vec::is_empty not const.
        self.words.is_empty() && self.long.is_none()
    }
    /// Returns the number of arguments contained by `self`.
    pub fn len(&self) -> usize {
        self.words.len() + self.long.is_some() as usize
    }
    /// Clears the argument list.
    ///
    /// This operation unconditionally deallocates memory.
    /// If you want to avoid this, use [`self.edit().clear()`][ArgsEditGuard::clear].
    pub fn clear(&mut self) {
        self.words = Cow::Owned(Vec::new());
        self.long = None;
    }
    /// Returns the length of `self` in bytes.
    ///
    /// This function is pessimistic and always counts the last argument as having a colon.
    pub fn len_bytes(&self) -> usize {
        let mut count = 0usize;
        for arg in self.words.iter() {
            count += arg.len() + 1;
        }
        if let Some(long) = &self.long {
            count += long.len() + 1;
        }
        count
    }
    /// Returns true if the last argument is more than one word long.
    pub const fn is_last_long(&self) -> bool {
        self.long.is_some()
    }
    /// Returns a slice of all the non-long aruments.
    pub fn words(&self) -> &[Arg<'a>] {
        self.words.as_ref()
    }
    /// Returns a slice of all the arguments as a slice of [`Arg`]s,
    /// or None if the final argument is long.
    pub fn all(&self) -> Option<&[Arg<'a>]> {
        self.long.is_none().then(|| self.words.as_ref())
    }
    /// Returns a slice of arguments with the last argument split off.
    pub fn split_last(&self) -> (&[Arg<'a>], Option<&Line<'a>>) {
        if let Some(long) = &self.long {
            (self.words.as_ref(), Some(long))
        } else if let Some((last, rest)) = self.words.split_last() {
            (rest, Some(last))
        } else {
            (&[], None)
        }
    }
}

impl<'a> From<Vec<Arg<'a>>> for Args<'a> {
    fn from(value: Vec<Arg<'a>>) -> Self {
        Args { words: Cow::Owned(value), long: None }
    }
}

impl std::fmt::Display for Args<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(long) = &self.long {
            for arg in self.words.iter() {
                write!(f, "{} ", arg)?;
            }
            write!(f, ":{}", long)
        } else if let Some((last, rest)) = self.words.split_last() {
            for arg in rest {
                write!(f, "{} ", arg)?;
            }
            write!(f, "{}", last)
        } else {
            Ok(())
        }
    }
}

#[cfg(feature = "serde")]
impl<'a> serde::Serialize for Args<'a> {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeSeq;
        let mut seq = ser.serialize_seq(Some(self.len()))?;
        for word in self.words.iter() {
            seq.serialize_element(word)?;
        }
        if let Some(long) = &self.long {
            seq.serialize_element(long)?;
        }
        seq.end()
    }
}

#[cfg(feature = "serde")]
impl<'a, 'de> serde::Deserialize<'de> for Args<'a> {
    fn deserialize<D>(de: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use crate::string::Bytes;
        use serde::de::Error;
        let mut args = Vec::<Bytes<'a>>::deserialize(de)?;
        let Some(last) = args.pop() else { return Ok(Args::empty()) };
        // Safety: Arg is a transparent newtype over Bytes.
        // However, even though it would be absurd for Vec to change layout
        // between two types with identical layout, Rust doesn't promise it won't happen.
        // Ergo, we're forced to copy the whole Vec out of caution.
        let mut words = Vec::with_capacity(args.len());
        for (idx, arg) in args.into_iter().enumerate() {
            if let Some(e) = Arg::find_invalid(&arg) {
                return Err(D::Error::custom(format!("invalid arg @ index {idx}: {e}")));
            }
            words.push(unsafe { Arg::from_unchecked(arg) });
        }
        let mut long = None;
        match last.try_into() {
            Ok(last) => add_impl(&mut words, &mut long, last),
            Err(e) => {
                return Err(D::Error::custom(format!("invalid arg @ index {}: {e}", words.len())))
            }
        };
        Ok(Args { words: Cow::Owned(words), long })
    }
}
