use super::{Transform, Utf8Policy};
use std::{
    borrow::Cow,
    ops::Deref,
    ptr::NonNull,
    sync::atomic::{AtomicI8, Ordering::Relaxed},
    sync::atomic::{AtomicUsize, Ordering},
};

/// Placeholder string for when some value cannot be displayed,
/// usually due to either being a non-UTF-8 string or secret.
pub const DISPLAY_PLACEHOLDER: &str = "<?>";

/// A borrowing or shared-owning immutable byte string. Not to be confused with Bytes
/// from the crate of the same name.
#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(from = "Vec<u8>", into = "Vec<u8>"))]
pub struct Bytes<'a> {
    value: &'a [u8],
    /// If this is Some, `value` points to data owned by this.
    /// It's very important that the slice never be returned with
    /// a lifetime longer than the IrcStr it was obvained from.
    ownership: Option<OwnedBytes>,
    /// The result of UTF-8 validity checks.
    /// 0 if "unknown", 1 if UTF-8, -1 if NOT UTF-8.
    utf8: AtomicI8,
}

impl<'a> Bytes<'a> {
    /// Returns a new empty `Bytes`.
    pub const fn empty() -> Bytes<'a> {
        Bytes { value: &[], ownership: None, utf8: AtomicI8::new(1) }
    }
    /// Cheaply converts a byte slice into a `Bytes`.
    pub const fn from_bytes(value: &'a [u8]) -> Bytes<'a> {
        Bytes { value, ownership: None, utf8: AtomicI8::new(0) }
    }
    /// Cheaply converts an `str` into a `Bytes`.
    pub const fn from_str(value: &'a str) -> Bytes<'a> {
        Bytes { value: value.as_bytes(), ownership: None, utf8: AtomicI8::new(1) }
    }
    /// Returns `true` if `self` is not borrowing its data.
    pub const fn is_owning(&self) -> bool {
        self.ownership.is_some()
    }
    /// Returns `true` if `self` is a sensitive byte-string.
    pub fn is_secret(&self) -> bool {
        self.ownership.as_ref().is_some_and(|o| o.is_secret())
    }
    /// Returns an owning version of this string.
    ///
    /// If this string already owns its data, this method only extends its lifetime.
    pub fn owning(self) -> Bytes<'static> {
        if self.is_owning() {
            // Lifetime extension.
            unsafe { std::mem::transmute(self) }
        } else {
            self.owning_force(false)
        }
    }
    /// Returns a secret version of this string.
    ///
    /// Secret strings' contents are not printed in formatting strings,
    /// whether using `Display` or `Debug`.
    /// Clones of secret strings are also secret.
    ///
    /// If the `zeroize` feature is enabled, these strings' buffers are zeroed out
    /// when the last reference to them is lost.
    ///
    /// For forward compatibility reasons, the value returned by this function
    /// has a lifetime bound of `'a`. It may be cheaply converted to `'static`
    /// using [`owning`][Bytes::owning]; secret strings are always owning.
    ///
    /// Currently, empty strings cannot be secret. This is a limitation that will be fixed.
    pub fn secret(self) -> Bytes<'a> {
        // Sneaky interior mutation.
        if self.ownership.as_ref().is_some_and(|o| o.set_secret()) {
            self
        } else {
            let retval = self.owning_force(true);
            retval
        }
    }
    /// Returns an owning version of this string, unconditionally copying data.
    fn owning_force(self, secret: bool) -> Bytes<'static> {
        unsafe {
            let (ownership, value) = OwnedBytes::from_vec(self.value.to_vec(), secret);
            Bytes { value, ownership, utf8: self.utf8.load(Relaxed).into() }
        }
    }
    // TODO: Are the "borrowed" methods from IrcStr needed?
    // They haven't really been necessary IME,
    // and with the UTF-8 checks they result in a lot of duplication,
    // especially if one finds a need for to_borrowed_or_cloned.

    /// Returns true if this byte string is empty.
    pub const fn is_empty(&self) -> bool {
        self.value.is_empty()
    }
    /// Returns the length of this byte string.
    pub const fn len(&self) -> usize {
        self.value.len()
    }
    /// Returns a reference to `self`'s value as a UTF-8 string if it's correctly encoded.
    ///
    /// This operation may do a UTF-8 validity check.
    /// If `self` was constructed from a UTF-8 string
    /// or a UTF-8 check was done previously, this check will be skipped.
    pub fn to_utf8(&self) -> Option<&str> {
        match self.utf8.load(Relaxed) {
            1 => Some(unsafe { std::str::from_utf8_unchecked(self.value) }),
            -1 => None,
            _ => {
                let so = std::str::from_utf8(self.value).ok();
                let utf8 = if so.is_some() { 1i8 } else { -1i8 };
                self.utf8.store(utf8, Relaxed);
                so
            }
        }
    }
    /// Returns a [`Cow`] `str` containing `self`'s value as a UTF-8 string
    /// with any non-UTF-8 byte sequences replaced with the
    /// [U+FFFD replacement character](std::char::REPLACEMENT_CHARACTER).
    pub fn to_utf8_lossy(&self) -> Cow<'_, str> {
        unsafe { self.utf8_cow() }
    }
    /// Returns `self` as a UTF-8 string,
    /// replacing any non-UTF-8 byte sequences with the the
    /// [U+FFFD replacement character](std::char::REPLACEMENT_CHARACTER).
    pub fn into_utf8_lossy(self) -> Self {
        match unsafe { self.utf8_cow() } {
            Cow::Borrowed(s) => {
                Bytes { value: s.as_bytes(), ownership: self.ownership, utf8: 1i8.into() }
            }
            Cow::Owned(o) => o.into(),
        }
    }
    #[cfg(feature = "base64")]
    fn to_base64_impl(&self) -> Bytes<'static> {
        use base64::engine::{general_purpose::STANDARD as ENGINE, Engine};
        let encoded = ENGINE.encode(self.value);
        unsafe {
            let (ownership, value) = OwnedBytes::from_vec(encoded.into_bytes(), self.is_secret());
            Bytes { value, ownership, utf8: 1i8.into() }
        }
    }
    /// Creates a base64-encoded version of this string.
    ///
    /// The returned string is always owning if it's non-empty.
    /// If `self` is secret, the returned string will also be secret.
    #[cfg(feature = "base64")]
    pub fn to_base64(&self) -> super::Word<'static> {
        unsafe { super::Word::from_unchecked(self.to_base64_impl()) }
    }
    /// Creates a base64-encoded version of this string,
    /// using the literal `"+"` if `self` is empty.
    ///
    /// The returned string is always owning if `self` is non-empty.
    /// If `self` is secret, the returned string will also be secret.
    #[cfg(feature = "base64")]
    pub fn to_base64_plus(&self) -> super::Arg<'static> {
        if self.is_empty() {
            crate::known::PLUS
        } else {
            unsafe { super::Arg::from_unchecked(self.to_base64_impl()) }
        }
    }
    unsafe fn utf8_cow(&self) -> Cow<'a, str> {
        match self.utf8.load(Relaxed) {
            1 => Cow::Borrowed(unsafe { std::str::from_utf8_unchecked(self.value) }),
            -1 => String::from_utf8_lossy(self.value),
            _ => {
                let sl = String::from_utf8_lossy(self.value);
                let utf8 = if matches!(&sl, Cow::Borrowed(_)) { 1i8 } else { -1i8 };
                self.utf8.store(utf8, Relaxed);
                sl
            }
        }
    }
    fn utf8_for_policy(&self, utf8: Utf8Policy) -> i8 {
        match utf8 {
            Utf8Policy::PreserveStrict => self.utf8.load(Relaxed),
            Utf8Policy::Preserve => (self.utf8.load(Relaxed) == 1) as i8,
            Utf8Policy::Invalid | Utf8Policy::Recheck | Utf8Policy::Valid => utf8 as i8,
        }
    }
    /// Returns `self`'s value as a slice.
    pub const fn as_bytes(&self) -> &[u8] {
        self.value
    }
    /// Returns `self`'s value as a slice with lifetime `'a`.
    ///
    /// # Safety
    /// If `self [is owning][Bytes::is_owning], then
    /// the returned slice must not outlive `self`.
    pub const unsafe fn as_bytes_unsafe(&self) -> &'a [u8] {
        self.value
    }
    /// Creates a clone of `self` using `value` as the value.
    ///
    /// # Safety
    /// If `self [is owning][Bytes::is_owning], then
    /// `value` must either point to data owned by `self` or an immutable static variable.
    /// A valid value can be obtained using [`as_bytes_unsafe`][Bytes::as_bytes_unsafe].
    ///
    /// The `utf8` parameter is trusted to be correct,
    /// and byte slices may be incorrectly cast unchecked to `str`s otherwise.
    pub unsafe fn using_value(&self, value: &'a [u8], utf8: Utf8Policy) -> Self {
        let utf8 = self.utf8_for_policy(utf8);
        let ownership = if value.is_empty() { None } else { self.ownership.clone() };
        Bytes { value, ownership, utf8: utf8.into() }
    }
    /// Updates `self` using the provided [`Transform`].
    pub fn transform<T: Transform>(&mut self, tf: T) -> T::Value<'a> {
        let tfed = tf.transform(self);
        if tfed.transformed.as_ref().is_empty() {
            *self = Bytes::empty();
            return tfed.value;
        }
        match tfed.transformed {
            Cow::Borrowed(s) => {
                match tfed.utf8 {
                    Utf8Policy::PreserveStrict => (),
                    Utf8Policy::Preserve => {
                        let _ = self.utf8.compare_exchange(-1i8, 0i8, Relaxed, Relaxed);
                    }
                    Utf8Policy::Invalid | Utf8Policy::Recheck | Utf8Policy::Valid => {
                        self.utf8.store(tfed.utf8 as i8, Relaxed);
                    }
                }
                self.value = s;
            }
            Cow::Owned(o) => {
                let utf8 = self.utf8_for_policy(tfed.utf8);
                unsafe {
                    let (ownership, value) = OwnedBytes::from_vec(o, self.is_secret());
                    *self = Bytes { value, ownership, utf8: utf8.into() };
                }
            }
        }
        tfed.value
    }
    /// Transforms `self` into a [`Vec`].
    ///
    /// This operation copies data unless `self` has sole ownership of its data.
    pub fn into_vec(self) -> Vec<u8> {
        if let Some(owner) = self.ownership {
            unsafe { owner.into_vec(self.value) }
        } else {
            self.value.to_vec()
        }
    }
}

impl<'a> Deref for Bytes<'a> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.value
    }
}

impl AsRef<[u8]> for Bytes<'_> {
    fn as_ref(&self) -> &[u8] {
        self.value
    }
}

impl std::borrow::Borrow<[u8]> for Bytes<'_> {
    fn borrow(&self) -> &[u8] {
        self.value
    }
}

// Conversions to IrcStr.

impl From<Vec<u8>> for Bytes<'static> {
    fn from(value: Vec<u8>) -> Self {
        unsafe {
            let (ownership, value) = OwnedBytes::from_vec(value, false);
            Bytes { value, ownership, utf8: 0i8.into() }
        }
    }
}

impl From<String> for Bytes<'static> {
    fn from(value: String) -> Self {
        unsafe {
            let (ownership, value) = OwnedBytes::from_vec(value.into_bytes(), false);
            Bytes { value, ownership, utf8: 1i8.into() }
        }
    }
}

impl<'a> From<&'a [u8]> for Bytes<'a> {
    fn from(value: &'a [u8]) -> Self {
        value.to_vec().into()
    }
}

impl<'a> From<&'a str> for Bytes<'a> {
    fn from(value: &'a str) -> Self {
        value.to_owned().into()
    }
}

impl<'a> From<Cow<'a, [u8]>> for Bytes<'a> {
    fn from(value: Cow<'a, [u8]>) -> Self {
        match value {
            Cow::Borrowed(s) => s.into(),
            Cow::Owned(s) => s.into(),
        }
    }
}

impl<'a> From<Cow<'a, str>> for Bytes<'a> {
    fn from(value: Cow<'a, str>) -> Self {
        match value {
            Cow::Borrowed(s) => s.into(),
            Cow::Owned(s) => s.into(),
        }
    }
}

impl<'a> From<Bytes<'a>> for Vec<u8> {
    fn from(value: Bytes<'a>) -> Self {
        value.into_vec()
    }
}

// Other impls.

impl Clone for Bytes<'_> {
    fn clone(&self) -> Self {
        Bytes {
            value: self.value,
            ownership: self.ownership.clone(),
            utf8: self.utf8.load(Relaxed).into(),
        }
    }
}

impl PartialEq for Bytes<'_> {
    fn eq(&self, b: &Bytes<'_>) -> bool {
        self.value == b.value
    }
}
impl<const N: usize> PartialEq<[u8; N]> for Bytes<'_> {
    fn eq(&self, other: &[u8; N]) -> bool {
        self.value == other.as_slice()
    }
}
impl<const N: usize> PartialEq<&[u8; N]> for Bytes<'_> {
    fn eq(&self, other: &&[u8; N]) -> bool {
        self == *other
    }
}
impl PartialEq<[u8]> for Bytes<'_> {
    fn eq(&self, other: &[u8]) -> bool {
        self.value == other
    }
}
impl PartialEq<&[u8]> for Bytes<'_> {
    fn eq(&self, other: &&[u8]) -> bool {
        self == *other
    }
}
impl PartialEq<str> for Bytes<'_> {
    fn eq(&self, other: &str) -> bool {
        // A proper UTF-8 validity check might be pointless here in many cases.
        self.utf8.load(Relaxed) != -1 && self.value == other.as_bytes()
    }
}
impl PartialEq<&str> for Bytes<'_> {
    fn eq(&self, other: &&str) -> bool {
        self == *other
    }
}

impl Eq for Bytes<'_> {}

impl PartialOrd for Bytes<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Bytes<'_> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.value.cmp(other.value)
    }
}

impl std::hash::Hash for Bytes<'_> {
    fn hash<H: std::hash::Hasher>(&self, hasher: &mut H) {
        self.value.hash(hasher);
    }
}

impl std::fmt::Display for Bytes<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_secret() {
            f.write_str(DISPLAY_PLACEHOLDER)
        } else if let Some(s) = self.to_utf8() {
            f.write_str(s)
        } else {
            f.write_str(DISPLAY_PLACEHOLDER)
        }
    }
}

impl std::fmt::Debug for Bytes<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (owning, secret) = if let Some(ref ownership) = self.ownership {
            (true, ownership.is_secret())
        } else {
            (false, false)
        };
        let mut f = f.debug_struct("Bytes");
        let f = f.field("owning", &owning);
        if !secret {
            if let Some(v) = self.to_utf8() {
                f.field("value", &v)
            } else {
                f.field("value", &self.value)
            }
        } else {
            f.field("value", &DISPLAY_PLACEHOLDER)
        }
        .finish()
    }
}

/// Implementation detail of Bytes.
/// A reference-counted dynamically-sized byte array.
///
/// It is designed to allow dirt-cheap conversions from Vec.
/// It returns a reference to the content on construction
/// but otherwise provides no access to the content.
/// There is also only a strong reference count, which is kept seperate.
struct OwnedBytes {
    rc: NonNull<AtomicUsize>,
    data: NonNull<u8>,
    size: usize,
}

impl OwnedBytes {
    /// Converts a Vec into Self (which owns the data)
    /// and a slice with an unbound lifetime (which does not).
    ///
    /// Returns `None` and an empty slice if `value` is empty.
    ///
    /// # Safety
    /// Unbound lifetimes are the devil.
    pub unsafe fn from_vec<'a>(mut value: Vec<u8>, secret: bool) -> (Option<Self>, &'a [u8]) {
        if value.is_empty() {
            return (None, Default::default());
        }
        // We're at the mercy of what the global allocator does here,
        // but at least this potentially does NOT copy.
        value.shrink_to_fit();
        // value is non-empty at this point, therefore the pointer is not null.
        let data = NonNull::new_unchecked(value.as_mut_ptr());
        let len = value.len();
        let size = value.capacity();
        std::mem::forget(value);
        let init_rc = if secret { 3usize } else { 2usize };
        // into_raw returns a non-null pointer.
        // https://doc.rust-lang.org/std/boxed/struct.Box.html#method.into_raw
        let rc = NonNull::new_unchecked(Box::into_raw(Box::new(AtomicUsize::new(init_rc))));
        let retval = OwnedBytes { rc, data, size };
        (Some(retval), std::slice::from_raw_parts(data.as_ptr().cast_const(), len))
    }
    /// Marks this byte string as sensitive if and only if `self` has exclusive ownership.
    #[must_use]
    pub fn set_secret(&self) -> bool {
        unsafe {
            let rc = self.rc.as_ref();
            rc.compare_exchange(2, 3, Ordering::Relaxed, Ordering::Relaxed).is_ok()
        }
    }
    #[must_use]
    pub fn is_secret(&self) -> bool {
        unsafe {
            let rc = self.rc.as_ref();
            rc.load(Ordering::Relaxed) & 1 != 0
        }
    }
    /// Attempts to re-use the buffer for constructing a `Vec` from `slice`.
    ///
    /// # Safety
    /// `slice` is assumed to be a slice of the data owned by self.
    pub unsafe fn into_vec(self, slice: &[u8]) -> Vec<u8> {
        let slice_start = slice.as_ptr();
        let data_start = self.data.as_ptr();
        let rc = self.rc.as_ref();
        if slice_start == data_start && rc.fetch_sub(2, Ordering::Release) < 4 {
            Vec::from_raw_parts(data_start, slice.len(), self.size)
        } else {
            slice.to_vec()
        }
    }
}

impl Clone for OwnedBytes {
    fn clone(&self) -> Self {
        let rc = unsafe { self.rc.as_ref() };
        rc.fetch_add(2, Ordering::Relaxed);
        OwnedBytes { rc: self.rc, data: self.data, size: self.size }
    }
}

impl Drop for OwnedBytes {
    fn drop(&mut self) {
        unsafe {
            let rc = self.rc.as_ref();
            let count = rc.fetch_sub(2, Ordering::Release);
            if count < 4 {
                let ptr = self.data.as_ptr();
                #[cfg(feature = "zeroize")]
                if count & 1 != 0 {
                    use zeroize::Zeroize;
                    std::slice::from_raw_parts_mut(ptr, self.size).zeroize();
                }
                std::mem::drop(Vec::from_raw_parts(ptr, 0, self.size));
                std::mem::drop(Box::from_raw(self.rc.as_mut()));
            }
        }
    }
}

// Should be send-safe and sync-safe since `data` is never written to after-construction
// unless the atomic ref count indicates exclusive ownership.
unsafe impl Send for OwnedBytes {}
unsafe impl Sync for OwnedBytes {}

// Unfortunately, address sanitizer support in rustc is still unstable.
// https://github.com/rust-lang/rust/issues/39699
