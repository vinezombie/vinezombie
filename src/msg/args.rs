//! IRC message argument utilities.

use crate::{IrcStr, IrcWord};

/// IRC message argument array.
///
/// This type enforces the invariant that
/// only the last argument may be longer than one word.
#[derive(Clone, PartialEq, Eq, Default, Debug)]
pub struct Args<'a>(Vec<IrcStr<'a>>, bool);

impl<'a> Args<'a> {
    /// Creates a new empty argument array.
    pub fn new() -> Args<'a> {
        Default::default()
    }
    /// Parses an argument array from a string.
    pub fn parse(s: impl Into<IrcStr<'a>>) -> Args<'a> {
        let mut s = s.into();
        let mut args = Args::default();
        loop {
            s.slice(str::trim_start);
            if s.lex_char(|c| *c == ':').is_some() {
                args.add(std::mem::take(&mut s));
                break;
            } else if let Some(word) = s.lex_word() {
                args.add_word(word);
            } else {
                break;
            }
        }
        args
    }
    /// Clears the argument array.
    pub fn clear(&mut self) {
        self.0.clear();
        self.1 = false;
    }
    /// Removes the last argument and returns it, or None is the argument array is empty.
    pub fn pop(&mut self) -> Option<IrcStr<'a>> {
        self.1 = false;
        self.0.pop()
    }
    /// Adds a word to the argument array.
    ///
    /// The word will be added to the end of the argument array unless the last argument is long,
    /// in which case it will be added just before it.
    pub fn add_word<'b: 'a>(&mut self, w: impl Into<IrcWord<'b>>) {
        let mut s: IrcStr<'a> = w.into().into();
        if self.1 {
            std::mem::swap(&mut s, self.0.last_mut().unwrap());
        }
        self.0.push(s);
    }
    /// Adds a word to the argument array in the second-to-last position.
    ///
    /// If the argument array is empty, just adds the word.
    /// Use this when the last argument in the array has special meaning.
    pub fn add_word_before_last<'b: 'a>(&mut self, w: impl Into<IrcWord<'b>>) {
        let mut s: IrcStr<'a> = w.into().into();
        if let Some(last) = self.0.last_mut() {
            std::mem::swap(&mut s, last);
        }
        self.0.push(s);
    }
    /// Adds a string to the end of this argument array.
    ///
    /// If the last string in the argument array is long, it will be replaced.
    pub fn add<'b: 'a>(&mut self, s: impl Into<IrcStr<'b>>) {
        let s = s.into();
        let long = !s.is_word();
        if self.1 {
            *self.0.last_mut().unwrap() = s;
        } else {
            self.0.push(s);
        }
        self.1 = long;
    }
    /// Returns true if there are no arguments.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    /// Returns true if the last argument is more than one word long.
    pub fn is_last_long(&self) -> bool {
        self.1
    }
    /// Returns a slice of all of the arguments.
    pub fn all(&self) -> &[IrcStr<'a>] {
        self.0.as_slice()
    }
    /// Returns a slice of arguments that are single-word arguments.
    pub fn words(&self) -> &[IrcWord<'a>] {
        match self.0.split_last() {
            Some((l, rest)) if !l.is_word() => unsafe {
                // IrcWord and IrcStr are transmutable between each other.
                IrcWord::cast_slice(rest)
            },
            _ => unsafe { IrcWord::cast_slice(self.0.as_slice()) },
        }
    }
    /// Returns the arguments with the last argument split off.
    pub fn split_last(&self) -> (&[IrcWord<'a>], Option<&IrcStr<'a>>) {
        if let Some((last, rest)) = self.0.split_last() {
            (unsafe { IrcWord::cast_slice(rest) }, Some(last))
        } else {
            (Default::default(), None)
        }
    }
    /// Returns the arguments with the last argument split off and mutable.
    pub fn split_last_mut(&mut self) -> (&[IrcWord<'a>], Option<&mut IrcStr<'a>>) {
        if let Some((last, rest)) = self.0.split_last_mut() {
            (unsafe { IrcWord::cast_slice(rest) }, Some(last))
        } else {
            (Default::default(), None)
        }
    }
}

impl std::fmt::Display for Args<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut pad = false;
        let (words, long) =
            if self.is_last_long() { self.split_last() } else { (self.words(), None) };
        for arg in words {
            if pad {
                write!(f, " ")?;
            }
            write!(f, "{}", arg)?;
            pad = true;
        }
        if let Some(c) = long {
            if pad {
                write!(f, " ")?;
            }
            write!(f, ":{}", c)?;
        }
        Ok(())
    }
}
