use super::BytesNewtype;
use crate::error::InvalidByte;

/// Type for creating [`Bytes`][crate::string::Bytes] newtypes by splitting strings.
#[derive(Clone, Copy, Debug)]
pub struct Splitter<T> {
    string: T,
    range: Range,
}

// TODO: Encoding might become part of the public API.

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
enum Encoding {
    #[default]
    Unknown,
    Utf8,
    Ascii,
}

#[derive(Clone, Copy, Debug)]
struct Range {
    pub start: usize,
    pub end: usize,
    pub encoding: Encoding,
}

/// Return type of [`Splitter::save()`].
#[derive(Debug)]
pub struct SavedIndices<'a, T> {
    splitter: &'a mut Splitter<T>,
    saved_range: Range,
    preserve: (bool, bool),
}

impl<'a, T> std::ops::Deref for SavedIndices<'a, T> {
    type Target = Splitter<T>;

    fn deref(&self) -> &Self::Target {
        self.splitter
    }
}
impl<'a, T> std::ops::DerefMut for SavedIndices<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.splitter
    }
}
impl<'a, T> Drop for SavedIndices<'a, T> {
    fn drop(&mut self) {
        let (start, end) = self.preserve;
        self.splitter.range.restore(self.saved_range, start, end);
    }
}

impl Range {
    pub fn len(&self) -> usize {
        self.end - self.start
    }
    pub fn consume(&mut self, mut count: usize, end: bool) -> usize {
        count = std::cmp::min(count, self.len());
        if end {
            self.end = self.start + count;
        } else {
            self.start += count;
        }
        if count > 0 && self.encoding == Encoding::Utf8 {
            self.encoding = Encoding::Unknown;
        }
        count
    }
    pub fn restore(&mut self, saved: Range, start: bool, end: bool) {
        if start {
            self.start = saved.start;
        }
        if end {
            self.end = saved.end;
        }
        if start && end {
            self.encoding = saved.encoding;
        }
    }
    pub fn constrain<'a, T>(&self, slice: &'a [T]) -> &'a [T] {
        &slice[self.start..self.end]
    }
}

impl<T> Splitter<T> {
    /// Returns `true` if this splitter is known to contain valid UTF-8.
    ///
    /// This method may return `false` even though the splitter does contain valid UTF-8.
    /// If you need to be sure, call [`check_encoding()`][Splitter::check_encoding]
    /// and check for a return value of `Ok(())`.
    pub fn is_utf8_lazy(&self) -> bool {
        self.range.encoding >= Encoding::Utf8
    }
    /// Returns a guard for this splitter that restores the start or end indices when
    /// it goes out of scope.
    pub fn save(&mut self, start: bool, end: bool) -> SavedIndices<'_, T> {
        let saved_range = self.range;
        SavedIndices { splitter: self, saved_range, preserve: (start, end) }
    }
    /// As [`save(false, true)`][Splitter::save].
    pub fn save_end(&mut self) -> SavedIndices<'_, T> {
        self.save(false, true)
    }
    /// Returns `true` if there are no more bytes to extract from `self`.
    pub fn is_empty(&self) -> bool {
        self.range.len() == 0
    }
    /// Returns how many bytes can be extracted from `self`.
    pub fn len(&self) -> usize {
        self.range.len()
    }
}

impl<T: AsRef<[u8]>> Splitter<T> {
    /// Checks `self`'s UTF-8 validity.
    pub fn check_encoding(&mut self) -> Result<(), std::str::Utf8Error> {
        if self.range.encoding < Encoding::Ascii {
            let slice = self.range.constrain(self.as_ref());
            let idx = slice.iter().position(|c| !c.is_ascii());
            self.range.encoding = if let Some(idx) = idx {
                let slice = &slice[idx..];
                std::str::from_utf8(slice)?;
                Encoding::Utf8
            } else {
                Encoding::Ascii
            }
        }
        Ok(())
    }
    /// View the slice.
    pub fn as_slice(&self) -> &[u8] {
        self.range.constrain(self.string.as_ref())
    }
    /// Gets the next byte without consuming it.
    pub fn peek_byte(&self) -> Option<u8> {
        self.as_slice().first().cloned()
    }
    /// Gets the next byte.
    pub fn next_byte(&mut self) -> Option<u8> {
        let retval = self.peek_byte();
        if let Some(byte) = retval {
            self.range.start += 1;
            if !byte.is_ascii() {
                self.range.encoding = Encoding::Unknown;
            }
        }
        retval
    }
    /// Removes leading bytes that are invalid for `U`.
    pub fn consume_invalid<'a, U: BytesNewtype<'a>>(&mut self) {
        let slice = self.range.constrain(self.as_ref());
        // Safety: We trust that U invalidity only happens on UTF-8 character boundries.
        if let Some(idx) = slice.iter().position(|c| !U::is_invalid(c)) {
            self.range.start += idx;
        } else {
            self.range.start = self.range.end;
        }
    }
    /// Removes leading ASCII whitespace.
    pub fn consume_whitespace(&mut self) {
        let slice = self.range.constrain(self.as_ref());
        if let Some(idx) = slice.iter().position(|c| !c.is_ascii_whitespace()) {
            self.range.start += idx;
        } else {
            self.range.start = self.range.end;
        }
    }
    /// Truncates the slice after and including the first byte for which `f` returns true.
    pub fn until<F: FnMut(&u8) -> bool>(&mut self, f: F) -> &mut Self {
        if let Some(idx) = self.as_slice().iter().position(f) {
            self.range.consume(idx, true);
        }
        self
    }
    /// Truncates the slice after and including the first byte which equals `byte`.
    pub fn until_byte(&mut self, byte: u8) -> &mut Self {
        if let Some(idx) = self.as_slice().iter().position(|b| *b == byte) {
            self.range.end = self.range.start + idx;
            if self.range.encoding == Encoding::Utf8 && !byte.is_ascii() {
                self.range.encoding = Encoding::Unknown;
            }
        }
        self
    }
    /// Truncates the slice if it is longer than `len` bytes.
    pub fn until_count(&mut self, len: usize) -> &mut Self {
        self.range.consume(len, true);
        self
    }
}

impl<'a, T: BytesNewtype<'a>> Splitter<T> {
    /// Creates a new splitter.
    pub fn new(string: T) -> Splitter<T> {
        let utf8 = string.is_utf8_lazy();
        let end = string.as_ref().len();
        let range = Range {
            start: 0,
            end,
            encoding: if utf8 { Encoding::Utf8 } else { Encoding::Unknown },
        };
        Self { string, range }
    }
    /// Returns `true` if string being split is sensitive.
    ///
    /// All [`Bytes`][crate::string::Bytes] and `Bytes` newtypes returned by
    /// `self`'s methods will be secret if this returns `true`.
    pub fn is_secret(&self) -> bool {
        self.string.is_secret()
    }
    unsafe fn as_slice_unsafe(&self) -> &'a [u8] {
        self.range.constrain(self.string.as_bytes_unsafe())
    }

    /// Gets the rest of the string without consuming it.
    ///
    /// This method is significantly more performant than
    /// [`self.peek_string(true)`][Splitter::peek_string]
    /// but is only defined for a subset of string types.
    pub fn peek_rest<U: BytesNewtype<'a> + From<T>>(&self) -> Result<U, InvalidByte> {
        if let Some(e) = U::check_others(self.as_slice()) {
            return Err(e);
        }
        unsafe {
            let bytes =
                self.string.using_value(self.as_slice_unsafe(), self.is_utf8_lazy()).into_bytes();
            Ok(U::from_unchecked(bytes))
        }
    }
    /// Gets the next string up to the next byte that is invalid for `U` without consuming it.
    ///
    /// If `require_rest` is true, errors if this string does not contain all the remaining
    /// bytes in `self.`
    pub fn peek_string<U: BytesNewtype<'a>>(&self, require_rest: bool) -> Result<U, InvalidByte> {
        unsafe {
            let mut slice = self.as_slice_unsafe();
            // Safety: We trust that U invalidity only happens on UTF-8 character boundries.
            slice = if let Some(idx) = slice.iter().position(U::is_invalid) {
                if !require_rest {
                    &slice[..idx]
                } else {
                    return Err(InvalidByte::new_at(slice, idx));
                }
            } else {
                slice
            };
            if let Some(e) = U::check_others(slice) {
                return Err(e);
            }
            let bytes = self.string.using_value(slice, self.is_utf8_lazy()).into_bytes();
            Ok(U::from_unchecked(bytes))
        }
    }
    /// Gets the rest of the string.
    ///
    /// On success, this splitter will be empty.
    /// On error, no bytes will be consumed.
    ///
    /// This method is significantly more performant than
    /// [`self.string(true)`][Splitter::string]
    /// but is only defined for a subset of string types.
    pub fn rest<U: BytesNewtype<'a> + From<T>>(&mut self) -> Result<U, InvalidByte> {
        let rest = self.peek_rest::<U>()?;
        self.range.start += rest.as_ref().len();
        Ok(rest)
    }
    /// Consumes this splitter to get the rest of the string, or default.
    ///
    /// This method is significantly more performant than
    /// [`self.string_or_default(true)`][Splitter::string_or_default]
    /// but is only defined for a subset of string types.
    pub fn rest_or_default<U: BytesNewtype<'a> + From<T> + Default>(&mut self) -> U {
        let rest = self.peek_rest::<U>().unwrap_or_default();
        self.range.start += rest.as_ref().len();
        rest
    }
    /// Gets the next string up to the next byte that is invalid for `U`.
    ///
    /// If `require_rest` is true, errors if this string does not contain all the remaining
    /// bytes in `self.`
    pub fn string<U: BytesNewtype<'a>>(&mut self, require_rest: bool) -> Result<U, InvalidByte> {
        let next = self.peek_string::<U>(require_rest)?;
        self.range.start += next.as_ref().len();
        Ok(next)
    }
    /// Gets the next string up to the next byte that is invalid for `U`, or default.
    ///
    /// If `require_rest` is true, returns the default if the string would not
    /// contain all the remaining bytes in `self.`
    /// This generally means returning an empty string and not consuming anything.
    pub fn string_or_default<U: BytesNewtype<'a> + Default>(&mut self, require_rest: bool) -> U {
        let next = self.peek_string::<U>(require_rest).unwrap_or_default();
        self.range.start += next.as_ref().len();
        next
    }
}

impl<T: AsRef<[u8]>> AsRef<[u8]> for Splitter<T> {
    fn as_ref(&self) -> &[u8] {
        self.string.as_ref()
    }
}

impl<'a, T: AsRef<[u8]>> AsRef<[u8]> for SavedIndices<'a, T> {
    fn as_ref(&self) -> &[u8] {
        self.splitter.as_ref()
    }
}

impl<T: AsRef<[u8]>> Iterator for Splitter<T> {
    type Item = u8;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_byte()
    }
}

impl<'a, T: AsRef<[u8]>> Iterator for SavedIndices<'a, T> {
    type Item = u8;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_byte()
    }
}
