use super::{Transform, Utf8Policy};
use std::{
    borrow::Cow,
    ops::Deref,
    ptr::NonNull,
    sync::atomic::{AtomicI8, Ordering::Relaxed},
    sync::atomic::{AtomicUsize, Ordering},
};

/// A borrowing or shared-owning immutable byte string. Not to be confused with Bytes
/// from the crate of the same name.
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
}

impl<'a> Bytes<'a> {
    /// Cheaply converts a byte slice into a `Bytes`.
    pub const fn from_bytes(value: &'a [u8]) -> Bytes<'a> {
        Bytes { value, ownership: None, utf8: AtomicI8::new(0) }
    }
    /// Cheaply converts an `str` into a `Bytes`.
    pub const fn from_str(value: &'a str) -> Bytes<'a> {
        Bytes { value: value.as_bytes(), ownership: None, utf8: AtomicI8::new(1) }
    }
    /// Return an owning version of this string.
    ///
    /// If this string already owns its data, this method only extends its lifetime.
    pub fn owning(&self) -> Bytes<'static> {
        if self.ownership.is_some() {
            // Lifetime extension.
            unsafe { std::mem::transmute(self.clone()) }
        } else {
            unsafe {
                let (owned, value) = OwnedBytes::from_vec(self.value.to_vec());
                Bytes { value, ownership: Some(owned), utf8: self.utf8.load(Relaxed).into() }
            }
        }
    }
    // TODO: Are the "borrowed" methods from IrcStr needed?
    // They haven't really been necessary IME,
    // and with the UTF-8 checks they result in a lot of duplication,
    // especially if one finds a need for to_borrowed_or_cloned.

    /// Returns true if this byte string is empty.
    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }
    /// Returns the length of this byte string.
    pub fn len(&self) -> usize {
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
    /// Returns a clone of `self` as a UTF-8 string,
    /// replacing any non-UTF-8 byte sequences with the the
    /// [U+FFFD replacement character](std::char::REPLACEMENT_CHARACTER).
    pub fn to_utf8_lossy(&self) -> Self {
        let update = match self.utf8.load(Relaxed) {
            1 => Cow::Borrowed(unsafe { std::str::from_utf8_unchecked(self.value) }),
            -1 => String::from_utf8_lossy(self.value),
            _ => {
                let sl = String::from_utf8_lossy(self.value);
                let utf8 = if matches!(&sl, Cow::Borrowed(_)) { 1i8 } else { -1i8 };
                self.utf8.store(utf8, Relaxed);
                sl
            }
        };
        match update {
            Cow::Borrowed(s) => {
                Bytes { value: s.as_bytes(), ownership: self.ownership.clone(), utf8: 1i8.into() }
            }
            Cow::Owned(o) => o.into(),
        }
    }
    /// Updates `self` using the provided [`Transform`].
    pub fn transform<T: Transform + ?Sized>(&mut self, tf: &T) -> T::Value {
        let tfed = tf.transform(self.value);
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
                let utf8 = match tfed.utf8 {
                    Utf8Policy::PreserveStrict => self.utf8.load(Relaxed),
                    Utf8Policy::Preserve => (self.utf8.load(Relaxed) == 1) as i8,
                    Utf8Policy::Invalid | Utf8Policy::Recheck | Utf8Policy::Valid => {
                        tfed.utf8 as i8
                    }
                };
                unsafe {
                    let (owned, value) = OwnedBytes::from_vec(o);
                    *self = Bytes { value, ownership: Some(owned), utf8: utf8.into() };
                }
            }
        }
        tfed.value
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
            let (owned, value) = OwnedBytes::from_vec(value);
            Bytes { value, ownership: Some(owned), utf8: 0i8.into() }
        }
    }
}

impl From<String> for Bytes<'static> {
    fn from(value: String) -> Self {
        unsafe {
            let (owned, value) = OwnedBytes::from_vec(value.into_bytes());
            Bytes { value, ownership: Some(owned), utf8: 1i8.into() }
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

impl PartialEq<[u8]> for Bytes<'_> {
    fn eq(&self, other: &[u8]) -> bool {
        self.value == other
    }
}

impl PartialEq for Bytes<'_> {
    fn eq(&self, b: &Bytes<'_>) -> bool {
        self.value == b.value
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
        f.write_str(&String::from_utf8_lossy(self.as_ref()))
    }
}

impl std::fmt::Debug for Bytes<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let owning = self.ownership.is_some();
        let mut f = f.debug_struct("StrBytes");
        let f = f.field("owning", &owning);
        if let Some(v) = self.to_utf8() {
            f.field("value", &v)
        } else {
            f.field("value", &self.value)
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
    /// # Safety
    /// Unbound lifetimes are the devil.
    pub unsafe fn from_vec<'a>(mut value: Vec<u8>) -> (Self, &'a [u8]) {
        // We're at the mercy of what the global allocator does here,
        // but at least this potentially does NOT copy.
        value.shrink_to_fit();
        // SAFE: as_mut_ptr returns a dangling pointer valid for 0-size reads
        // if the vector did not allocate.
        // https://doc.rust-lang.org/std/vec/struct.Vec.html#method.as_mut_ptr
        let data = NonNull::new_unchecked(value.as_mut_ptr());
        let len = value.len();
        let size = value.capacity();
        std::mem::forget(value);
        // SAFE: into_raw returns a non-null pointer.
        // https://doc.rust-lang.org/std/boxed/struct.Box.html#method.into_raw
        let rc = NonNull::new_unchecked(Box::into_raw(Box::new(AtomicUsize::new(1))));
        let retval = OwnedBytes { rc, data, size };
        (retval, std::slice::from_raw_parts(data.as_ptr().cast_const(), len))
    }
}

impl Clone for OwnedBytes {
    fn clone(&self) -> Self {
        let rc = unsafe { self.rc.as_ref() };
        rc.fetch_add(1, Ordering::Relaxed);
        OwnedBytes { rc: self.rc, data: self.data, size: self.size }
    }
}

impl Drop for OwnedBytes {
    fn drop(&mut self) {
        unsafe {
            let rc = self.rc.as_ref();
            if rc.fetch_sub(1, Ordering::Release) == 1 {
                std::mem::drop(Vec::from_raw_parts(self.data.as_ptr(), 0, self.size));
                std::mem::drop(Box::from_raw(self.rc.as_mut()));
            }
        }
    }
}

// Unfortunately, address sanitizer support in rustc is still unstable.
// https://github.com/rust-lang/rust/issues/39699
