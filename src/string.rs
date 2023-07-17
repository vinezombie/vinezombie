//! Byte strings and string manipulation utilities.
//!
//! The core primitive of vinezombie is [an immutable byte string type][Bytes]
//! that can either borrow or have shared ownership of its contents.
//! This primitive also features optimizations for checking UTF-8 validity.
//!
//! There are several newtypes based around this primitive,
//! each with progressively greater restrictions.
//!
//! Accompanying `Bytes` and its newtypes are a family of [`Transformation`]s
//! which allow for rich string manipulation while upholding the newtypes' invariants.

// TODO: SVG link.

#[cfg(feature = "base64")]
pub mod base64;
mod builder;
mod bytes;
#[cfg(feature = "serde")]
mod serde;
//mod splitter;
mod subtypes;
#[cfg(test)]
mod tests;
pub mod tf;

pub use builder::*;
pub use bytes::*;
pub use subtypes::*;
//pub use splitter::*;

use std::borrow::Cow;

/// Types that represent byte string tranformations.
///
/// # Safety
/// This trait is hiliariously unsafe. Do not implement it yourself.
///
/// `'a` may be a forged lifetime that does not correctly represent the lifetime of the data
/// it references. `self` must NOT store anything with a lifetime bounded by `'a`.
///
/// `Transformation::transformed`, if it borrows, must either
/// point to data owned by `self` or an immutable static variable.
///
/// The type used for `Value` must either not use `'a`,
/// or every use of `'a` in its definition must ultimately be `Bytes<'a>`.
///
/// The `utf8` field of the returned [`Transformation`] is trusted to be correct,
/// and byte slices may be incorrectly cast unchecked to `str`s otherwise.
pub unsafe trait Transform {
    /// The type of values yielded in addition to a transformed byte string.
    type Value<'a>;
    /// Transforms a byte string.
    fn transform<'a>(self, bytes: &Bytes<'a>) -> Transformation<'a, Self::Value<'a>>;
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

impl<'a, T> Transformation<'a, T> {
    /// Returns a transformed version of input data where the entire input has been consumed.
    pub fn empty(value: T) -> Self {
        Transformation {
            value,
            transformed: Cow::Borrowed(Default::default()),
            utf8: Utf8Policy::Valid,
        }
    }
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
