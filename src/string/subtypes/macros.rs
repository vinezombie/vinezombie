macro_rules! check_bytes {
    ($bytes:ident, $f:expr) => {{
        let mut i = 0usize;
        while i < $bytes.len() {
            if $f(&$bytes[i]) {
                return Some(InvalidString::Byte($bytes[i]));
            }
            i += 1;
        }
        None
    }};
}

/// # Safety
/// Here be transmutes. $ssuper must be either Bytes
/// or a an $sname from a previous use of this macro.
macro_rules! impl_subtype {
    (
        $doc:literal
        $sname:ident: $ssuper:ident
        $tname:ident: $tsuper:ident
        $bcheck:expr;
        $ocheck:expr;
        |$sarg:ident| $sbody:block
    ) => {
        impl_subtype! {
            $doc
            $sname: $ssuper
            $tname: $tsuper
            $bcheck;
            $ocheck;
        }
        impl<'a> $sname<'a> {
            /// Tries to convert `sup` into an instance of this type.
            /// Errors if `sup` does not uphold this type's guarantees.
            pub fn from_super(sup: impl Into<$ssuper<'a>>) -> Result<Self, InvalidString> {
                let sup = sup.into();
                #[inline]
                fn check($sarg: &[u8]) -> Option<InvalidString> {
                    $sbody
                }
                if let Some(e) = check(sup.as_ref()) {
                    Err(e)
                } else {
                    Ok(unsafe { std::mem::transmute(sup) })
                }
            }
            /// Cheaply converts `self` into the next more-general type in the string hierarchy.
            pub const fn into_super(self) -> $ssuper<'a> {
                // Can't use `self.0` for non-const destructor reasons.
                unsafe { std::mem::transmute(self) }
            }
        }
    };
    (
        $doc:literal
        $sname: ident: $ssuper:ident
        $tname:ident: $tsuper:ident
        $bcheck:expr;
        $ocheck:expr;
    ) => {
        #[doc = $doc]
        #[doc = ""]
        #[doc = "See the [module-level documentation][super] for more."]
        #[repr(transparent)]
        #[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
        #[cfg_attr(feature = "serde", derive(serde_derive::Serialize))]
        #[cfg_attr(feature = "serde", serde(into = "Bytes<'a>"))]
        pub struct $sname<'a>(Bytes<'a>);

        #[doc = concat!("Marker for [`", stringify!($sname), "`]-safe [`Transform`]s.")]
        #[doc = ""]
        #[doc = "# Safety"]
        #[doc = "[`Transform::transform()`]' must return a byte string that maintains"]
        #[doc = concat!("[`",stringify!($sname),"`]'s invariants.")]
        #[doc = "See its struct-level documentation for more info."]
        pub unsafe trait $tname: $tsuper {}

        impl<'a> $sname<'a> {
            /// Returns `true` if this string cannot contain `byte`.
            #[must_use]
            #[inline]
            pub const fn is_invalid(byte: &u8) -> bool {
                $bcheck(byte)
            }
            /// Returns the first byte and its index that violate this type's guarantees.
            #[inline]
            pub const fn find_invalid(bytes: &[u8]) -> Option<InvalidString> {
                // Optimization: the block here can also do a test for ASCII-validity
                // and use that to infer UTF-8 validity.
                if let Some(e) = $ocheck(bytes) {
                    return Some(e);
                }
                check_bytes!(bytes, $bcheck)
            }
            /// Tries to convert `bytes` into an instance of this type.
            /// Errors if `bytes` does not uphold this type's guarantees.
            pub fn from_bytes(bytes: impl Into<Bytes<'a>>) -> Result<Self, InvalidString> {
                let bytes = bytes.into();
                if let Some(e) = Self::find_invalid(bytes.as_ref()) {
                    Err(e)
                } else {
                    Ok($sname(bytes))
                }
            }
            /// Tries to convert the provided [`str`] into an instance of this type.
            ///
            /// This is generally intended to be used for string literals,
            /// as in addition to being `const` it panics.
            /// However, it can be used for any string reference.
            ///
            /// # Panics
            /// Panics if `string` does not uphold this type's gurarantees.
            pub const fn from_str(string: &'a str) -> Self {
                if Self::find_invalid(string.as_bytes()).is_some() {
                    // Can't emit the error here because of the const context.
                    panic!("invalid string")
                } else {
                    unsafe { Self::from_unchecked(Bytes::from_str(string)) }
                }
            }
            /// Performs an unchecked conversion from `bytes`.
            ///
            /// # Safety
            /// This function assumes that this type's guarantees are upheld by `bytes`.
            pub const unsafe fn from_unchecked(bytes: Bytes<'a>) -> Self {
                $sname(bytes)
            }
            /// Tries to convert `value` into an owning, secret instance of this type.
            /// Errors if `value` does not uphold this type's guarantees.
            pub fn from_secret(value: Vec<u8>) -> Result<Self, InvalidString> {
                if let Some(e) = Self::find_invalid(value.as_ref()) {
                    std::mem::drop(crate::string::SecretBuf::from(value));
                    Err(e)
                } else {
                    Ok(Self(Bytes::from_secret(value)))
                }
            }
            /// Transforms `self` using the provided [`Transform`]
            /// that upholds `self`'s invariant.
            pub fn transform<T: $tname>(&mut self, tf: T) -> T::Value {
                self.0.transform(tf)
            }
            /// Cheaply converts `self` into the underlying byte string.
            pub const fn into_bytes(self) -> Bytes<'a> {
                // Can't use `self.0` for non-const destructor reasons.
                unsafe { std::mem::transmute(self) }
            }
            /// Returns `true` if `self` is not borrowing its data.
            pub const fn is_owning(&self) -> bool {
                self.0.is_owning()
            }
            /// Returns `true` if `self` is a sensitive byte-string.
            pub fn is_secret(&self) -> bool {
                self.0.is_secret()
            }
            /// Returns an owning version of this string.
            ///
            /// If this string already owns its data, this method only extends its lifetime.
            pub fn owning(self) -> $sname<'static> {
                $sname(self.0.owning())
            }
            /// Returns a secret version of this string.
            ///
            /// See [`Bytes::secret`] for information on what this means.
            pub fn secret(self) -> $sname<'a> {
                $sname(self.0.secret())
            }
            /// Returns true if this byte string is empty.
            pub const fn is_empty(&self) -> bool {
                self.0.is_empty()
            }
            /// Returns the length of this byte string.
            pub const fn len(&self) -> usize {
                self.0.len()
            }
            /// Converts `self` into a [CString][std::ffi::CString].
            ///
            /// This is safe and infallible due to the invariant of [`NoNul`],
            /// which is inherited by every other [`Bytes`] pseudosubtype in this crate.
            pub fn into_cstring(self) -> std::ffi::CString {
                let vec = self.into();
                unsafe { std::ffi::CString::from_vec_unchecked(vec) }
            }
            /// Borrows `self` as a slice of [`NonZeroU8`][std::num::NonZeroU8]s.
            ///
            /// This is safe and infallible due to the invariant of [`NoNul`],
            /// which is inherited by every other [`Bytes`] pseudosubtype in this crate.
            pub fn as_bytes_nonzero(&self) -> &[std::num::NonZeroU8] {
                unsafe { std::mem::transmute(self.as_bytes()) }
            }
        }
        unsafe impl<'a> BytesNewtype<'a> for $sname<'a> {
            unsafe fn as_bytes_unsafe(&self) -> &'a [u8] {
                self.0.as_bytes_unsafe()
            }
            fn check_others(bytes: &[u8]) -> Option<InvalidString> {
                $ocheck(bytes)
            }
            unsafe fn from_unchecked(bytes: Bytes<'a>) -> Self {
                Self::from_unchecked(bytes)
            }
            fn into_bytes(self) -> Bytes<'a> {
                self.into_bytes()
            }
            fn into_vec(this: Self::This<'_>) -> Vec<u8> {
                this.into()
            }
            fn is_invalid(byte: &u8) -> bool {
                $bcheck(byte)
            }
            fn is_utf8_lazy(&self) -> bool {
                self.0.is_utf8_lazy().unwrap_or_default()
            }
            unsafe fn using_value(&self, bytes: &'a [u8], utf8: bool) -> Self {
                use crate::string::Utf8Policy;
                let utf8 = if utf8 { Utf8Policy::Valid } else { Utf8Policy::Recheck };
                let bytes = self.0.using_value(bytes, utf8);
                Self::from_unchecked(bytes)
            }
            fn is_secret(&self) -> bool {
                self.0.is_secret()
            }
        }
        impl<'a> From<$sname<'a>> for Bytes<'a> {
            fn from(value: $sname<'a>) -> Bytes<'a> {
                value.into_bytes()
            }
        }
        impl<'a> From<$sname<'a>> for Vec<u8> {
            fn from(value: $sname<'a>) -> Vec<u8> {
                value.into_bytes().into_vec()
            }
        }
        impl<'a> TryFrom<Bytes<'a>> for $sname<'a> {
            type Error = InvalidString;
            fn try_from(value: Bytes<'a>) -> Result<$sname<'a>, InvalidString> {
                $sname::from_bytes(value)
            }
        }
        impl AsRef<[u8]> for $sname<'_> {
            fn as_ref(&self) -> &[u8] {
                self.0.as_ref()
            }
        }
        impl Borrow<[u8]> for $sname<'_> {
            fn borrow(&self) -> &[u8] {
                self.0.borrow()
            }
        }
        impl<'a> std::ops::Deref for $sname<'a> {
            type Target = $ssuper<'a>;
            fn deref(&self) -> &Self::Target {
                unsafe { &*(self as *const Self as *const Self::Target) }
            }
        }
        impl std::fmt::Display for $sname<'_> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(f)
            }
        }
        impl<const N: usize> PartialEq<[u8; N]> for $sname<'_> {
            fn eq(&self, other: &[u8; N]) -> bool {
                self.0 == *other
            }
        }
        impl<const N: usize> PartialEq<&[u8; N]> for $sname<'_> {
            fn eq(&self, other: &&[u8; N]) -> bool {
                self.0 == *other
            }
        }
        impl PartialEq<[u8]> for $sname<'_> {
            fn eq(&self, other: &[u8]) -> bool {
                self.0 == *other
            }
        }
        impl PartialEq<&[u8]> for $sname<'_> {
            fn eq(&self, other: &&[u8]) -> bool {
                self.0 == *other
            }
        }
        impl PartialEq<str> for $sname<'_> {
            fn eq(&self, other: &str) -> bool {
                self.0 == *other
            }
        }
        impl PartialEq<&str> for $sname<'_> {
            fn eq(&self, other: &&str) -> bool {
                self.0 == *other
            }
        }
        impl<'a> TryFrom<&'a [u8]> for $sname<'a> {
            type Error = InvalidString;
            fn try_from(value: &'a [u8]) -> Result<$sname<'a>, Self::Error> {
                Bytes::from(value).try_into()
            }
        }
        impl<'a> TryFrom<&'a str> for $sname<'a> {
            type Error = InvalidString;
            fn try_from(value: &'a str) -> Result<$sname<'a>, Self::Error> {
                Bytes::from(value).try_into()
            }
        }
        impl<'a> TryFrom<Vec<u8>> for $sname<'a> {
            type Error = InvalidString;
            fn try_from(value: Vec<u8>) -> Result<$sname<'static>, Self::Error> {
                Bytes::from(value).try_into()
            }
        }
        impl<'a> TryFrom<String> for $sname<'a> {
            type Error = InvalidString;
            fn try_from(value: String) -> Result<$sname<'static>, Self::Error> {
                Bytes::from(value).try_into()
            }
        }
        #[cfg(feature = "serde")]
        impl<'a, 'de> serde::Deserialize<'de> for $sname<'a> {
            fn deserialize<D>(de: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                use serde::de::Error;
                let bytes = Bytes::deserialize(de)?;
                bytes.try_into().map_err(D::Error::custom)
            }
        }
        unsafe impl<'a> crate::owning::MakeOwning for $sname<'a> {
            type This<'x> = $sname<'x>;

            fn make_owning(&mut self) {
                self.0.make_owning()
            }
        }
    };
}

macro_rules! conversions {
    ($sname: ident: $ssuper: ident) => {
        // TODO: Downcasting TryFrom impls?
        impl<'a> From<$sname<'a>> for $ssuper<'a> {
            fn from(value: $sname<'a>) -> $ssuper<'a> {
                unsafe { std::mem::transmute(value) }
            }
        }
        impl<'a> TryFrom<$ssuper<'a>> for $sname<'a> {
            type Error = InvalidString;
            fn try_from(value: $ssuper<'a>) -> Result<$sname<'a>, InvalidString> {
                $sname::from_bytes(value.into_bytes())
            }
        }
    };
}

macro_rules! maybe_empty {
    ($sname:ident) => {
        impl<'a> Default for $sname<'a> {
            fn default() -> Self {
                $sname(Bytes::default())
            }
        }
        impl<'a> $sname<'a> {
            #[doc = concat!("Returns a new empty `", stringify!($sname), "`.")]
            pub const fn empty() -> Self {
                $sname(Bytes::empty())
            }
        }
    };
}
