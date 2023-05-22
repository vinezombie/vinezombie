//! IRC message argument utilities.

use crate::string::{
    tf::{SplitFirst, SplitWord, TrimStart},
    Arg, Line,
};

#[inline(always)]
unsafe fn downcast_line_slice<'a, 's>(lines: &'s [Line<'a>]) -> &'s [Arg<'a>] {
    &*(lines as *const [Line<'a>] as *const [Arg<'a>])
}

/// IRC message argument array.
///
/// This type enforces the invariant that
/// only the last argument may be longer than one word.
#[derive(Clone, PartialEq, Eq, Hash, Default, Debug)]
pub struct Args<'a>(Vec<Line<'a>>, bool);

impl<'a> Args<'a> {
    /// Creates a new empty argument array.
    pub const fn new() -> Args<'a> {
        Args(Vec::new(), false)
    }
    /// Converts `self` into a version that owns its data.
    pub fn owning(self) -> Args<'static> {
        // rustc optimizations mean that the map operation should run in-place.
        Args(self.0.into_iter().map(Line::owning).collect(), self.1)
    }
    /// Parses an argument array from a string.
    pub fn parse(line: impl Into<Line<'a>>) -> Args<'a> {
        let mut line = line.into();
        let mut args = Args::new();
        loop {
            line.transform(TrimStart(|b: &u8| *b == b' '));
            if matches!(line.first(), Some(b':')) {
                line.transform(SplitFirst);
                args.0.push(line);
                args.1 = true;
                break;
            }
            let word = line.transform(SplitWord);
            if word.is_empty() {
                break;
            }
            args.0.push(word.into());
        }
        args
    }
    /// Clears the argument array.
    pub fn clear(&mut self) {
        self.0.clear();
        self.1 = false;
    }
    /// Removes the last argument and returns it, or `None` is the argument array is empty.
    pub fn pop(&mut self) -> Option<Line<'a>> {
        self.1 = false;
        self.0.pop()
    }
    /// Adds a word to the argument array.
    ///
    /// The word will be added to the end of the argument array unless the last argument is long,
    /// in which case it will be added just before it.
    pub fn add<'b: 'a>(&mut self, w: impl Into<Arg<'b>>) {
        let mut s: Line = w.into().into();
        if self.1 {
            std::mem::swap(&mut s, self.0.last_mut().unwrap());
        }
        self.0.push(s);
    }
    /// Adds a word to the argument array in the second-to-last position.
    ///
    /// If the argument array is empty, just adds the word.
    /// Use this when the last argument in the array has special meaning.
    pub fn add_before_last<'b: 'a>(&mut self, w: impl Into<Arg<'b>>) {
        let mut s: Line = w.into().into();
        if let Some(last) = self.0.last_mut() {
            std::mem::swap(&mut s, last);
        }
        self.0.push(s);
    }
    /// Adds a string to the end of this argument array.
    ///
    /// If the last string in the argument array is long, it will be replaced.
    pub fn add_long<'b: 'a>(&mut self, s: impl Into<Line<'b>>) {
        let s = s.into();
        let long = Arg::find_invalid(&s).is_some();
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
    pub fn all(&self) -> &[Line<'a>] {
        self.0.as_slice()
    }
    /// Returns a slice of arguments that are single-word arguments.
    pub fn args(&self) -> &[Arg<'a>] {
        match self.0.split_last() {
            Some((l, rest)) if Arg::find_invalid(l).is_some() => unsafe {
                downcast_line_slice(rest)
            },
            _ => unsafe { downcast_line_slice(&self.0) },
        }
    }
    /// Returns the arguments with the last argument split off.
    pub fn split_last(&self) -> (&[Arg<'a>], Option<&Line<'a>>) {
        if let Some((last, rest)) = self.0.split_last() {
            (unsafe { downcast_line_slice(rest) }, Some(last))
        } else {
            (Default::default(), None)
        }
    }
    /// Returns the arguments with the last argument split off and mutable.
    pub fn split_last_mut(&mut self) -> (&[Arg<'a>], Option<&mut Line<'a>>) {
        if let Some((last, rest)) = self.0.split_last_mut() {
            (unsafe { downcast_line_slice(rest) }, Some(last))
        } else {
            (Default::default(), None)
        }
    }
}

impl std::fmt::Display for Args<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut pad = false;
        let (words, long) =
            if self.is_last_long() { self.split_last() } else { (self.args(), None) };
        for arg in words {
            if pad {
                write!(f, " ")?;
            }
            write!(f, "{}", arg)?;
            pad = true;
        }
        if let Some(last) = long {
            if pad {
                write!(f, " ")?;
            }
            write!(f, ":{}", last)?;
        }
        Ok(())
    }
}
