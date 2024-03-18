//! Utilities for advanced control over ownership.
//!
//! std is very limited in what it offers, and in the interest of being resource-efficient,
//! this library comes with some of its own methods of playing games with ownership.

/// Types that have at least one "owning" state where they own all their data,
/// one "borrowing" state where they do not.
///
/// # Safety
/// By implementing this trait, a type promises that after calling make_owning,
/// nothing in it actually borrows, and we can transmute it to extend the lifetime without
/// ever allowing safe code to get a reference with this extended lifetime.
///
/// For instance, `Bytes<'a>` is very careful not to hand out `&'a`s to safe code
/// without checking that it is in is borrowing state first.
pub unsafe trait MakeOwning {
    /// `Self`, parameterized by lifetime.
    type This<'x>;

    /// Converts `self` into an owning state if it is not already.
    fn make_owning(&mut self);
}

// Wouldn't it be grand if we could blanket-impl ^ for every type that has no lifetime arguments?

unsafe impl<'a, T: std::borrow::ToOwned + ?Sized + 'static> MakeOwning for std::borrow::Cow<'a, T> {
    type This<'x> = std::borrow::Cow<'x, T>;

    fn make_owning(&mut self) {
        if let std::borrow::Cow::Borrowed(b) = self {
            let owned = b.to_owned();
            *self = std::borrow::Cow::Owned(owned);
        }
    }
}

// Due to stable borrow checker limitations, this is largely useless without `-Zpolonius`.
// This function is unlikely to see use prior to the 2024 edition.
// See https://blog.rust-lang.org/inside-rust/2023/10/06/polonius-update.html
// Until then, vinezombie's implementos of `MakeOwning` all have `owning()` methods
// that do the same thing. They're just not as cool.
/*
/// Converts a non-owning value into an owning value and forges its lifetime argument.
///
/// This is safe to do assuming `T` correctly implements the unsafe [`MakeOwning`] trait.
pub fn owning<'b, T: MakeOwning>(mut value: T) -> T::This<'b>
where for<'a> T: MakeOwning<This<'a> = T> {
    value.make_owning();
    unsafe {std::mem::transmute(value)}
}
*/
