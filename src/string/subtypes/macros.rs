/// # Safety
/// Here be transmutes. $ssuper must be either Bytes
/// or a an $sname from a previous use of this macro.
macro_rules! impl_subtype {
    (
        $doc:literal
        $sname:ident: $ssuper:ident
        $tname:ident: $tsuper:ident
        $bcheck:expr;
        |$targ:ident| $tbody:block
        |$uarg:ident| $ubody:block
    ) => {
        impl_subtype! {
            $doc
            $sname: $ssuper
            $tname: $tsuper
            $bcheck;
            |$targ| $tbody
        }
        impl<'a> $sname<'a> {
            /// Tries to convert `sup` into an instance of this type.
            /// Errors if `sup` does not uphold this type's guarantees.
            pub fn from_super(sup: impl Into<$ssuper<'a>>) -> Result<Self, InvalidByte> {
                let sup = sup.into();
                #[inline]
                fn check($uarg: &[u8]) -> Option<InvalidByte> {
                    $ubody
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
        |$targ:ident| $tbody:block
    ) => {
        #[doc = $doc]
        #[repr(transparent)]
        #[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize))]
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
            pub const fn find_invalid(bytes: &[u8]) -> Option<InvalidByte> {
                // Optimization: the block here can also do a test for ASCII-validity
                // and use that to infer UTF-8 validity.
                let $targ = bytes;
                $tbody
            }
            /// Tries to convert `bytes` into an instance of this type.
            /// Errors if `bytes` does not uphold this type's guarantees.
            pub fn from_bytes(bytes: impl Into<Bytes<'a>>) -> Result<Self, InvalidByte> {
                let bytes = bytes.into();
                if let Some(e) = Self::find_invalid(bytes.as_ref()) {
                    Err(e)
                } else {
                    Ok($sname(bytes))
                }
            }
            /// Tries to convert the provided [`str`] into an instance of this type.
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
            pub fn from_secret(value: Vec<u8>) -> Result<Self, InvalidByte> {
                if let Some(e) = Self::find_invalid(value.as_ref()) {
                    #[cfg(feature = "zeroize")]
                    std::mem::drop(zeroize::Zeroizing::new(value));
                    Err(e)
                } else {
                    Ok(Self(Bytes::from_secret(value)))
                }
            }
            /// Transforms `self` using the provided [`Transform`]
            /// that upholds `self`'s invariant.
            pub fn transform<T: $tname>(&mut self, tf: T) -> T::Value<'a> {
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
                self.0.is_owning()
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
            type Error = InvalidByte;
            fn try_from(value: Bytes<'a>) -> Result<$sname<'a>, InvalidByte> {
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
            type Error = InvalidByte;
            fn try_from(value: &'a [u8]) -> Result<$sname<'a>, Self::Error> {
                Bytes::from(value).try_into()
            }
        }
        impl<'a> TryFrom<&'a str> for $sname<'a> {
            type Error = InvalidByte;
            fn try_from(value: &'a str) -> Result<$sname<'a>, Self::Error> {
                Bytes::from(value).try_into()
            }
        }
        impl TryFrom<Vec<u8>> for $sname<'static> {
            type Error = InvalidByte;
            fn try_from(value: Vec<u8>) -> Result<$sname<'static>, Self::Error> {
                Bytes::from(value).try_into()
            }
        }
        impl TryFrom<String> for $sname<'static> {
            type Error = InvalidByte;
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
    };
}

/// Creates a builder for a Bytes newtype that is NOT
/// invalidated by appending bytes for which is_invalid returns false.
macro_rules! impl_builder {
    ($name:ident from $pname:ident with $tname:ident for $sname:ident default) => {
        impl_builder!($name from $pname with $tname for $sname);
        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }
        impl $name {
            /// Creates a new empty builder.
            pub const fn new() -> Self {
                Self{ bytes: Vec::new(), utf8: true }
            }
            /// Creates a new empty builder with the specified capacity.
            pub fn with_capacity(capacity: usize) -> Self {
                Self{ bytes: Vec::with_capacity(capacity), utf8: true }
            }
        }
    };
    ($name:ident from $pname:ident with $tname:ident for $sname:ident) => {
        #[doc = concat!("Builder for creating [`", stringify!($sname), "`]s.")]
        ///
        /// This type contains a `Vec` of bytes that upholds the string type's invariant.
        /// It also tracks UTF-8 validity.
        #[derive(Clone, Debug)]
        pub struct $name {
            bytes: Vec<u8>,
            utf8: bool
        }

        impl $name {
            /// Creates a new builder containing the provided initial value.
            pub fn new_from<'a>(init: impl Into<$sname<'a>>) -> Self {
                let init = init.into();
                let utf8 = init.is_utf8_lazy().unwrap_or_default();
                Self { bytes: init.into(), utf8 }
            }
            /// Shrinks the capacity of this builder as much as possible.
            pub fn shrink_to_fit(&mut self) {
                self.bytes.shrink_to_fit()
            }
            /// Ensures space for at least `bytes` additional bytes.
            /// May reserve additional space.
            ///
            /// See [`Vec::reserve`].
            pub fn reserve(&mut self, len: usize) {
                self.bytes.reserve(len)
            }
            /// Ensures space for at least `bytes` additional bytes.
            /// Reserves as little additional spaces as possible.
            ///
            /// See [`Vec::reserve_exact`].
            pub fn reserve_exact(&mut self, len: usize) {
                self.bytes.reserve_exact(len)
            }
            /// Checks `self`'s UTF-8 validity.
            pub fn check_utf8(&mut self) -> Result<(), std::str::Utf8Error> {
                if !self.utf8 {
                    std::str::from_utf8(&self.bytes)?;
                    self.utf8 = true;
                }
                Ok(())
            }
            /// Consumes `self` to build an owning byte string.
            pub fn build<'a>(self) -> $sname<'a> {
                unsafe {
                    if self.utf8 {
                        let string = String::from_utf8_unchecked(self.bytes);
                        $sname::from_unchecked(string.into())
                    } else {
                        $sname::from_unchecked(self.bytes.into())
                    }
                }
            }
            /// Appends `string` to the end of `self` without checking validity.
            ///
            /// `utf8` must be false unless `string` is entirely valid UTF-8.
            ///
            /// # Safety
            /// Misuse of this function can easily result in invariant violations
            /// or the construction of invalid UTF-8 strings that are assumed to be valid.
            /// It is your responsibility to ensure that `self`'s data
            /// is valid for [`
            #[doc = stringify!($sname)]
            /// `] after calling this function.
            pub unsafe fn append_unchecked(
                &mut self,
                string: impl AsRef<[u8]>,
                utf8: bool
            ) {
                let string = string.as_ref();
                self.utf8 &= utf8;
                self.bytes.extend_from_slice(string);
            }
            /// Adds `string` to the end of `self`.
            pub fn append<'a>(&mut self, string: impl Into<$pname<'a>>) {
                let string = string.into();
                unsafe {self.append_unchecked(
                    string.as_bytes(),
                    string.is_utf8_lazy().unwrap_or_default()
                )}
            }

            /// Checks `string`'s validity and adds it to the end of `self`.
            pub fn try_append(
                &mut self,
                string: impl AsRef<[u8]>
            ) -> Result<(), InvalidByte> {
                let string = string.as_ref();
                let mut idx = 0usize;
                let mut ascii = true;
                for byte in string {
                    if $sname::is_invalid(byte) {
                        return Err(InvalidByte::new(*byte, idx))
                    }
                    ascii &= byte.is_ascii();
                    idx += 1;
                }
                self.utf8 &= ascii;
                self.bytes.extend_from_slice(string);
                Ok(())
            }
            /// Checks `string`'s validity and adds it to the end of `self`.
            pub fn try_append_str(
                &mut self,
                string: impl AsRef<str>
            ) -> Result<(), InvalidByte> {
                let string = string.as_ref();
                let mut idx = 0usize;
                for byte in string.as_bytes() {
                    if $sname::is_invalid(byte) {
                        return Err(InvalidByte::new(*byte, idx))
                    }
                    idx += 1;
                }
                self.bytes.extend_from_slice(string.as_bytes());
                Ok(())
            }
            /// Tries to append a byte.
            pub fn try_push(&mut self, byte: u8) -> Result<(), InvalidByte> {
                if $sname::is_invalid(&byte) {
                    Err(InvalidByte::new(byte, self.bytes.len()))
                } else {
                    self.utf8 &= byte.is_ascii();
                    self.bytes.push(byte);
                    Ok(())
                }
            }
            /// Tries to append a `char`.
            pub fn try_push_char(&mut self, c: char) -> Result<(), InvalidByte> {
                let mut buf = [0u8; 4];
                self.try_append_str(c.encode_utf8(&mut buf))
            }
            /// Transforms `self` using the provided [`Transform`].
            ///
            /// This operation is slightly more-expensive than the equivalent operation
            /// on the string type this builder is for.
            pub fn transform<'a, T: $tname>(&mut self, tf: T) -> T::Value<'a> {
                let this = std::mem::replace(self, Self { bytes: Vec::new(), utf8: false });
                let mut this = this.build();
                let retval = this.transform(tf);
                *self = Self::new_from(this);
                retval
            }
        }

        impl std::ops::Deref for $name {
            type Target = [u8];
            fn deref(&self) -> &[u8] {
                &self.bytes
            }
        }
        impl AsRef<[u8]> for $name {
            fn as_ref(&self) -> &[u8] {
                &self.bytes
            }
        }

        impl<'a> From<$name> for $sname<'a> {
            fn from(value: $name) -> Self {
                value.build()
            }
        }

        // TODO: impl Extend a bunch.
    }
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
            type Error = InvalidByte;
            fn try_from(value: $ssuper<'a>) -> Result<$sname<'a>, InvalidByte> {
                $sname::from_bytes(value.into_bytes())
            }
        }
    };
}

macro_rules! check_bytes {
    ($bytes:ident, $f:expr) => {{
        let mut i = 0usize;
        while i < $bytes.len() {
            if $f(&$bytes[i]) {
                return Some(InvalidByte::new_at($bytes, i));
            }
            i += 1;
        }
        None
    }};
}
