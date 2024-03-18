use super::{Bytes, BytesNewtype};
use crate::error::InvalidString;

/// Type for creating [`Bytes`] newtypes by concatenation.
///
/// This type contains a `Vec` of bytes that upholds the string type's invariant.
/// It also tracks UTF-8 validity.
#[derive(Clone, Debug)]
pub struct Builder<T> {
    bytes: Vec<u8>,
    utf8: bool,
    marker: std::marker::PhantomData<T>,
}

impl<'a, T: BytesNewtype<'a>> Default for Builder<T>
where
    T::This<'a>: BytesNewtype<'a> + Default,
{
    fn default() -> Self {
        Builder::new(Default::default())
    }
}

impl<'a, T: BytesNewtype<'a>> Builder<T> {
    /// Creates a new builder containing the provided initial value.
    ///
    /// `T::This` is `T` with any lifetime.
    pub fn new<'b>(init: T::This<'b>) -> Self
    where
        T::This<'b>: BytesNewtype<'b>,
    {
        let utf8 = init.is_utf8_lazy();
        let bytes = T::into_vec(init);
        Self { bytes, utf8, marker: std::marker::PhantomData }
    }
    /// Shrinks the capacity of this builder as much as possible.
    pub fn shrink_to_fit(&mut self) {
        self.bytes.shrink_to_fit();
    }
    /// Ensures space for at least `bytes` additional bytes.
    /// May reserve additional space.
    ///
    /// See [`Vec::reserve`].
    pub fn reserve(&mut self, len: usize) {
        self.bytes.reserve(len);
    }
    /// Ensures space for at least `bytes` additional bytes.
    /// Reserves as little additional spaces as possible.
    ///
    /// See [`Vec::reserve_exact`].
    pub fn reserve_exact(&mut self, len: usize) {
        self.bytes.reserve_exact(len);
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
    pub fn build(self) -> T {
        unsafe {
            let bytes: Bytes = if self.utf8 {
                let string = String::from_utf8_unchecked(self.bytes);
                string.into()
            } else {
                self.bytes.into()
            };
            T::from_unchecked(bytes)
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
    /// is valid for `T` after calling this function.
    pub unsafe fn append_unchecked(&mut self, string: impl AsRef<[u8]>, utf8: bool) {
        let string = string.as_ref();
        self.utf8 &= utf8;
        self.bytes.extend_from_slice(string);
    }
    /// Adds `string` to the end of `self`.
    ///
    /// `T::This` is `T` with any lifetime.
    pub fn append<'b>(&mut self, string: impl Into<T::This<'b>>)
    where
        T::This<'b>: BytesNewtype<'b>,
    {
        let string = string.into();
        unsafe {
            let bytes = string.as_bytes_unsafe();
            self.append_unchecked(bytes, string.is_utf8_lazy());
        }
    }

    /// Checks `string`'s validity and adds it to the end of `self`.
    pub fn try_append(&mut self, string: impl AsRef<[u8]>) -> Result<(), InvalidString> {
        let string = string.as_ref();
        let mut ascii = true;
        for byte in string.iter() {
            if T::is_invalid(byte) {
                return Err(InvalidString::Byte(*byte));
            }
            ascii &= byte.is_ascii();
        }
        self.utf8 &= ascii;
        self.bytes.extend_from_slice(string);
        Ok(())
    }
    /// Checks `string`'s validity and adds it to the end of `self`.
    pub fn try_append_str(&mut self, string: impl AsRef<str>) -> Result<(), InvalidString> {
        let string = string.as_ref().as_bytes();
        for byte in string.iter() {
            if T::is_invalid(byte) {
                return Err(InvalidString::Byte(*byte));
            }
        }
        self.bytes.extend_from_slice(string);
        Ok(())
    }
    /// Tries to append a byte.
    pub fn try_push(&mut self, byte: u8) -> Result<(), InvalidString> {
        if T::is_invalid(&byte) {
            Err(InvalidString::Byte(byte))
        } else {
            self.utf8 &= byte.is_ascii();
            self.bytes.push(byte);
            Ok(())
        }
    }
    /// Tries to append a `char`.
    pub fn try_push_char(&mut self, c: char) -> Result<(), InvalidString> {
        let mut buf = [0u8; 4];
        self.try_append_str(c.encode_utf8(&mut buf))
    }
}

impl<T> std::ops::Deref for Builder<T> {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.bytes
    }
}

impl<T> AsRef<[u8]> for Builder<T> {
    fn as_ref(&self) -> &[u8] {
        &self.bytes
    }
}

// TODO: impl Extend a bunch.
