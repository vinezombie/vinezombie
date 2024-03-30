use super::{Transform, Utf8Policy};
use std::{
    borrow::Cow,
    ops::Deref,
    sync::atomic::{AtomicI8, Ordering::Relaxed},
};

/// Placeholder string for when some value cannot be displayed,
/// usually due to either being a non-UTF-8 string or secret.
pub const DISPLAY_PLACEHOLDER: &str = "<?>";

/// A borrowing or shared-owning immutable byte string.
///
/// See the [module-level documentation][super] for more.
#[derive(Default)]
pub struct Bytes<'a> {
    value: &'a [u8],
    /// If this is Some, `value` points to data owned by this.
    /// It's very important that the slice never be returned with
    /// a lifetime longer than the IrcStr it was obvained from.
    ownership: Option<OwnedBytes>,
    /// The result of UTF-8 validity checks.
    /// 0 if "unknown", 1 if UTF-8, -1 if NOT UTF-8.
    utf8: AtomicI8,
    /// Whether this string is "secret" or not.
    secret: bool,
}

impl Bytes<'static> {
    /// Returns a [`Cow`] `str` with `'static` lifetime containing `self`'s value as a UTF-8 string
    /// with any non-UTF-8 byte sequences replaced with the
    /// [U+FFFD replacement character](std::char::REPLACEMENT_CHARACTER).
    ///
    /// This is efficient for `Bytes` instances constructed out of string literals.
    pub fn to_utf8_lossy_static(&self) -> Cow<'static, str> {
        if self.is_owning() {
            Cow::Owned(self.to_utf8_lossy().into_owned())
        } else {
            unsafe { self.utf8_cow() }
        }
    }
}

unsafe impl<'a> crate::owning::MakeOwning for crate::string::Bytes<'a> {
    type This<'b> = crate::string::Bytes<'b>;

    fn make_owning(&mut self) {
        if !self.is_owning() {
            self.owning_force(false);
        }
    }
}

impl<'a> Bytes<'a> {
    /// Returns a new empty `Bytes`.
    pub const fn empty() -> Bytes<'a> {
        Bytes { value: &[], ownership: None, utf8: AtomicI8::new(1), secret: false }
    }
    /// Cheaply converts a byte slice into a `Bytes`.
    pub const fn from_bytes(value: &'a [u8]) -> Self {
        Bytes { value, ownership: None, utf8: AtomicI8::new(0), secret: false }
    }
    /// Cheaply converts an `str` into a `Bytes`.
    pub const fn from_str(value: &'a str) -> Self {
        Bytes { value: value.as_bytes(), ownership: None, utf8: AtomicI8::new(1), secret: false }
    }
    /// Cheaply conversts a secret value into a `Bytes`.
    pub fn from_secret(value: Vec<u8>) -> Self {
        let (ownership, value) = unsafe { OwnedBytes::from_vec(value) };
        Bytes { value, ownership, utf8: AtomicI8::new(0), secret: true }
    }
    /// Returns `true` if `self` is not borrowing its data.
    pub const fn is_owning(&self) -> bool {
        self.ownership.is_some()
    }
    /// Returns `true` if `self` is a sensitive byte-string.
    pub fn is_secret(&self) -> bool {
        self.secret
    }
    /// Returns an owning version of this string.
    ///
    /// If this string already owns its data, this method only extends its lifetime.
    pub fn owning<'b>(mut self) -> Bytes<'b> {
        if !self.is_owning() {
            let secret = self.secret;
            self.owning_force(secret);
        }
        // Lifetime extension.
        unsafe { std::mem::transmute(self) }
    }
    /// Returns a secret version of this string.
    ///
    /// Secret strings' contents are not printed in formatting strings,
    /// whether using `Display` or `Debug`.
    /// Clones of secret strings are also secret.
    ///
    /// If the `zeroize` feature is enabled, these strings' buffers are zeroed out
    /// when the last reference to them is lost.
    pub fn secret(mut self) -> Self {
        if !self.secret && self.is_owning() {
            self.owning_force(true);
        } else {
            self.secret = true;
        }
        self
    }
    fn owning_force(&mut self, secret: bool) {
        unsafe {
            let (ownership, value) = OwnedBytes::from_vec(self.value.to_vec());
            *self = Bytes { value, ownership, utf8: self.utf8.load(Relaxed).into(), secret }
        }
    }

    /// Returns true if this byte string is empty.
    pub const fn is_empty(&self) -> bool {
        self.value.is_empty()
    }
    /// Returns the length of this byte string.
    pub const fn len(&self) -> usize {
        self.value.len()
    }
    /// Checks if `self` is known to be UTF-8 without checking the whole string.
    ///
    /// This operation does not perform UTF-8 validity checks.
    #[inline]
    pub fn is_utf8_lazy(&self) -> Option<bool> {
        match self.utf8.load(Relaxed) {
            1 => Some(true),
            -1 => Some(false),
            _ => None,
        }
    }
    /// Returns a reference to `self`'s value as a UTF-8 string if it's correctly encoded.
    ///
    /// This operation may do a UTF-8 validity check.
    /// If `self` was constructed from a UTF-8 string
    /// or a UTF-8 check was done previously, this check will be skipped.
    pub fn to_utf8(&self) -> Option<&str> {
        match self.is_utf8_lazy() {
            Some(true) => Some(unsafe { std::str::from_utf8_unchecked(self.value) }),
            Some(false) => None,
            None => {
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
            Cow::Borrowed(s) => Bytes {
                value: s.as_bytes(),
                ownership: self.ownership,
                utf8: 1i8.into(),
                secret: self.secret,
            },
            Cow::Owned(o) => o.into(),
        }
    }
    #[cfg(feature = "base64")]
    fn to_base64_impl(&self) -> Bytes<'static> {
        use base64::engine::{general_purpose::STANDARD as ENGINE, Engine};
        let encoded = ENGINE.encode(self.value);
        unsafe {
            let (ownership, value) = OwnedBytes::from_vec(encoded.into_bytes());
            Bytes { value, ownership, utf8: 1i8.into(), secret: self.secret }
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
            crate::names::PLUS
        } else {
            unsafe { super::Arg::from_unchecked(self.to_base64_impl()) }
        }
    }
    unsafe fn utf8_cow(&self) -> Cow<'a, str> {
        match self.is_utf8_lazy() {
            Some(true) => Cow::Borrowed(unsafe { std::str::from_utf8_unchecked(self.value) }),
            Some(false) => String::from_utf8_lossy(self.value),
            None => {
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
        Bytes { value, ownership, utf8: utf8.into(), secret: self.secret }
    }
    /// Updates `self` using the provided [`Transform`].
    pub fn transform<T: Transform>(&mut self, tf: T) -> T::Value {
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
                    let (ownership, value) = OwnedBytes::from_vec(o);
                    *self = Bytes { value, ownership, utf8: utf8.into(), secret: self.secret };
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
            unsafe { owner.into_vec(self.value, self.secret) }
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

impl<'a> From<Vec<u8>> for Bytes<'a> {
    fn from(value: Vec<u8>) -> Self {
        unsafe {
            let (ownership, value) = OwnedBytes::from_vec(value);
            Bytes { value, ownership, utf8: 0i8.into(), secret: false }
        }
    }
}

impl<'a> From<String> for Bytes<'a> {
    fn from(value: String) -> Self {
        unsafe {
            let (ownership, value) = OwnedBytes::from_vec(value.into_bytes());
            Bytes { value, ownership, utf8: 1i8.into(), secret: false }
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

impl<'a> From<Bytes<'a>> for Cow<'a, [u8]> {
    fn from(value: Bytes<'a>) -> Self {
        match value.ownership {
            Some(v) => Cow::Owned(unsafe { v.into_vec(value.value, value.secret) }),
            None => Cow::Borrowed(value.value),
        }
    }
}

// Other impls.

impl Clone for Bytes<'_> {
    fn clone(&self) -> Self {
        Bytes {
            value: self.value,
            ownership: self.ownership.clone(),
            utf8: self.utf8.load(Relaxed).into(),
            secret: self.secret,
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
        let mut f = f.debug_struct("Bytes");
        let f = f.field("owning", &self.is_owning());
        if !self.is_secret() {
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
#[repr(transparent)]
#[derive(Clone)]
struct OwnedBytes(crate::util::ThinArc<crate::util::OwnedSlice<u8>>);

impl OwnedBytes {
    /// Converts a Vec into Self (which owns the data)
    /// and a slice with an unbound lifetime (which does not).
    ///
    /// Returns `None` and an empty slice if `value` is empty.
    ///
    /// # Safety
    /// Unbound lifetimes are the devil, and this returns a reference with one.
    pub unsafe fn from_vec<'a>(value: Vec<u8>) -> (Option<Self>, &'a [u8]) {
        let (os, slice) = crate::util::OwnedSlice::from_vec(value);
        (os.map(|os| OwnedBytes(crate::util::ThinArc::new(os))), slice)
    }
    /// Attempts to re-use the buffer for constructing a `Vec` from `slice`.
    ///
    /// # Safety
    /// `slice` is assumed to be a slice of the data owned by self.
    pub unsafe fn into_vec(self, slice: &[u8], _secret: bool) -> Vec<u8> {
        if let Ok(os) = self.0.try_unwrap() {
            let (retval, _destroy) = os.into_vec(slice);
            #[cfg(feature = "zeroize")]
            if _secret {
                if let Some(destroy) = _destroy {
                    destroy.zeroize_drop();
                }
            }
            retval
        } else {
            slice.to_vec()
        }
    }
}

// Should be send-safe and sync-safe since `data` is never written to after-construction
// unless the atomic ref count indicates exclusive ownership.
unsafe impl Send for OwnedBytes {}
unsafe impl Sync for OwnedBytes {}

// Unfortunately, address sanitizer support in rustc is still unstable.
// https://github.com/rust-lang/rust/issues/39699
