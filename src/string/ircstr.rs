use std::{borrow::Cow, ops::Deref, sync::Arc};

use crate::IrcWord;

#[inline]
fn owned_from_buf(data: &str) -> IrcStr<'static> {
    if data.is_empty() {
        return IrcStr::default();
    }
    let arc: Arc<str> = data.into();
    owned_from_rc(arc)
}
#[inline]
fn owned_from_rc(arc: Arc<str>) -> IrcStr<'static> {
    let slice = unsafe { Arc::as_ptr(&arc).as_ref().unwrap_unchecked() };
    IrcStr(slice, Some(arc))
}
/*
#[inline]
fn mutated(data: &str, f: impl FnOnce(&mut str)) -> IrcStr<'static> {
    let mut arc: Arc<str> = data.into();
    // TODO: get_mut_unchecked
    f(unsafe { Arc::get_mut(&mut arc).unwrap_unchecked() });
    owned_from_arc(arc)
}
*/

/// A borrowing or shared-owning immutable UTF-8 string.
#[derive(Clone, Default)]
pub struct IrcStr<'a>(&'a str, Option<Arc<str>>);
// ^ Inner slice, if the option is Some, refers to data owned by that Arc.
// It's very important that the slice never be returned with
// a lifetime longer than the IrcStr it was obtained from.

impl<'a> IrcStr<'a> {
    /// Returns true if this string owns its data.
    pub fn is_owning(&self) -> bool {
        self.1.is_some()
    }
    /// Return an owning version of this string.
    ///
    /// If this string already owns its data, this method only extends its lifetime.
    pub fn owning(&self) -> IrcStr<'static> {
        if self.is_owning() {
            // Lifetime extension.
            unsafe { std::mem::transmute(self.clone()) }
        } else {
            owned_from_buf(self.0)
        }
    }
    /// Returns a reference to the borrowed string with the same lifetime as the outer IrcStr.
    pub fn as_str_borrowed(&self) -> Option<&'a str> {
        (!self.is_owning()).then_some(self.0)
    }
    /// Shrinks this string using the provided function.
    pub fn slice<F>(&mut self, f: F) -> &mut Self
    where
        F: FnOnce(&str) -> &str,
    {
        // TODO: Is there a soundness hole here?
        self.0 = unsafe { std::mem::transmute(f(self.as_ref())) };
        self
    }
    /// Parses data from some or all of this string.
    pub fn parse<T, E, F>(&mut self, f: F) -> Result<T, E>
    where
        F: FnOnce(&str) -> Result<(T, &str), E>,
    {
        let (t, s) = f(self.as_ref())?;
        // TODO: Is there a soundness hole here?
        self.0 = unsafe { std::mem::transmute(s) };
        Ok(t)
    }
    /// Extracts a slice from the string.
    pub fn lex<'b, F>(&'b mut self, f: F) -> Option<Self>
    where
        F: FnOnce(&'b str) -> Option<(&'b str, &'b str)>,
    {
        let (a, b) = f(self.0 as &'b str)?;
        // TODO: Is there a soundness hole here?
        self.0 = unsafe { std::mem::transmute(b) };
        let a = unsafe { std::mem::transmute(a) };
        Some(IrcStr(a, self.1.clone()))
    }
    /// Extracts a character from the string.
    pub fn lex_char<F>(&mut self, f: F) -> Option<char>
    where
        F: FnOnce(&char) -> bool,
    {
        let mut chars = self.chars();
        let c = chars.next();
        let s = chars.as_str();
        match c {
            Some(c) if f(&c) => {
                // TODO: Is there a soundness hole here?
                self.0 = unsafe { std::mem::transmute(s) };
                Some(c)
            }
            _ => None,
        }
    }
    /// Extracts a word from the string.
    pub fn lex_word(&mut self) -> Option<IrcWord<'a>> {
        self.lex(|s| {
            let mut splitter = s.splitn(2, |c: char| c.is_ascii_whitespace());
            let w = splitter.next().filter(|w| !w.is_empty())?;
            let r = splitter.next().unwrap_or("");
            Some((w, r))
        })
        .map(|s| unsafe { IrcWord::new_unchecked(s) })
    }
    /// Returns true if this string is non-empty, contains no spaces, and doesn't begin with ':'.
    pub fn is_word(&self) -> bool {
        let Some(c) = self.as_bytes().get(0) else {
            return false;
        };
        *c != b':' && !self.chars().any(|c| c.is_ascii_whitespace())
    }
    /// Returns the length of this string in bytes.
    pub fn len_bytes(&self) -> usize {
        self.0.as_bytes().len()
    }
}

impl<'a> Deref for IrcStr<'a> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl AsRef<str> for IrcStr<'_> {
    fn as_ref(&self) -> &str {
        self.0
    }
}

impl std::borrow::Borrow<str> for IrcStr<'_> {
    fn borrow(&self) -> &str {
        self.0
    }
}

// Conversions to IrcStr.

impl From<String> for IrcStr<'static> {
    fn from(v: String) -> Self {
        let rc: Arc<str> = v.into_boxed_str().into();
        owned_from_rc(rc)
    }
}

impl<'a> From<&'a str> for IrcStr<'a> {
    fn from(v: &'a str) -> Self {
        IrcStr(v, None)
    }
}

impl<'a> From<Cow<'a, str>> for IrcStr<'a> {
    fn from(v: Cow<'a, str>) -> Self {
        match v {
            Cow::Borrowed(s) => s.into(),
            Cow::Owned(s) => s.into(),
        }
    }
}

// Other impls.

impl PartialEq<&str> for IrcStr<'_> {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl PartialEq for IrcStr<'_> {
    fn eq(&self, b: &IrcStr<'_>) -> bool {
        self.0 == b.0
    }
}

impl Eq for IrcStr<'_> {}

impl PartialOrd for IrcStr<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for IrcStr<'_> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(other.0)
    }
}

impl std::hash::Hash for IrcStr<'_> {
    fn hash<H: std::hash::Hasher>(&self, hasher: &mut H) {
        self.0.hash(hasher)
    }
}

impl std::fmt::Display for IrcStr<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_ref())
    }
}

impl std::fmt::Debug for IrcStr<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
