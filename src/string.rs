//! Byte strings and string manipulation utilities.
//!
//! The core primitive of vinezombie is an immutable byte string type
//! that can either borrow or have shared ownership of its contents.

//#[cfg(feature = "base64")]
//pub mod base64;
mod bytes;
//pub mod strmap;

pub use bytes::Bytes;
//pub use ircstr::IrcStr;
//pub use ircword::IrcWord;

use std::borrow::Cow;

/// Types that represent byte string tranformations.
///
/// # Safety
/// `bytes` may have a forged lifetime, often `'static`.
/// Implementors MUST NOT store anything with a lifetime that is bounded by `'a`.
///
/// The `utf8` field of the returned [`Transformation`] is trusted to be correct,
/// and byte slices may be incorrectly cast unchecked to `str`s otherwise.
pub unsafe trait Transform {
    /// The type of values yielded in addition to a transformed byte string.
    type Value;
    /// Transforms a byte string.
    fn transform<'a>(&self, bytes: &'a [u8]) -> Transformation<'a, Self::Value>;
}

/// The result of a byte string transformation, as returned by [`Transform::transform()`].
pub struct Transformation<'a, T> {
    /// An additional value yielded by this transformation. Often `()`.
    pub value: T,
    /// The transformed string.
    pub transformed: Cow<'a, [u8]>,
    /// The UTF-8 validity of `transformed`. See [`Utf8Policy`].
    pub utf8: Utf8Policy,
}

/// The UTF-8 validity of a transformed string based on the input string.
#[repr(i8)]
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum Utf8Policy {
    /// The returned slice is NOT valid UTF-8.
    Invalid = -1,
    /// The returned slice has unknown UTF-8 validity.
    #[default]
    Recheck = 0,
    /// The returned slice is valid UTF-8.
    Valid = 1,
    /// The returned slice is valid UTF-8 if the input slice was valid UTF-8.
    Preserve,
    /// The returned slice is valid UTF-8 if and only if the input slice was valid UTF-8.
    PreserveStrict,
}
