use super::IrcStr;

/// One-word-only newtype around IrcStr.
///
/// This type upholds the invariant that the underlying string is exactly one word
/// that does not begin with a colon, as defined by [IrcStr::is_word].
#[repr(transparent)]
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct IrcWord<'a>(IrcStr<'a>);

impl<'a> IrcWord<'a> {
    // TODO: mut variant when there's a point to it.
    // TODO: Public functions for slice casting?
    pub(crate) unsafe fn cast_slice<'b>(s: &'b [IrcStr<'a>]) -> &'b [Self] {
        std::mem::transmute(s)
    }
    /// Creates a new IrcWord without checking that the string is a word.
    ///
    /// # Safety
    /// The provided string must be a word as defined by [IrcStr::is_word]
    /// in order do not violate invariants.
    ///
    /// Misuse of this function is unlikely to cause UB,
    /// but it may result in sending malformed IRC messages.
    pub unsafe fn new_unchecked(s: impl Into<IrcStr<'a>>) -> IrcWord<'a> {
        IrcWord(s.into())
    }
    /// Creates a new IrcWord if the input string is a word.
    pub fn new(s: impl Into<IrcStr<'a>>) -> Option<IrcWord<'a>> {
        let s = s.into();
        s.is_word().then_some(IrcWord(s))
    }
    /// Returns true.
    ///
    /// This method exists to shadow [IrcStr::is_word] during deref coersion.
    pub fn is_word(&self) -> bool {
        true
    }
    /// As [IrcStr::owning].
    pub fn owning(&self) -> IrcWord<'static> {
        IrcWord(self.0.owning())
    }
}

impl<'a> std::ops::Deref for IrcWord<'a> {
    type Target = IrcStr<'a>;
    fn deref(&self) -> &IrcStr<'a> {
        &self.0
    }
}

/// Panicking conversion from a static str.
///
/// # Panics
/// Panics if the provided string is not a single word.
impl From<&'static str> for IrcWord<'static> {
    fn from(s: &'static str) -> IrcWord<'static> {
        IrcWord::new(s).expect("&'static str to IrcWord conversion")
    }
}

impl<'a> From<IrcWord<'a>> for IrcStr<'a> {
    fn from(s: IrcWord<'a>) -> IrcStr<'a> {
        s.0
    }
}

impl PartialEq<&str> for IrcWord<'_> {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl std::fmt::Display for IrcWord<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
