use std::{io::Write, num::NonZeroU8};

// Don't repr(transparent) Numeric.

/// A three-digit numeric reply code.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde_derive::Serialize, serde_derive::Deserialize))]
#[cfg_attr(feature = "serde", serde(into = "u16"))]
pub struct Numeric([NonZeroU8; 3]);

impl AsRef<str> for Numeric {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::hash::Hash for Numeric {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Manual impl to be certain the hash matches that from hashing a slice with digits.
        self.as_bytes().as_slice().hash(state);
    }
}

impl std::borrow::Borrow<[u8]> for Numeric {
    fn borrow(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl AsRef<[u8]> for Numeric {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl std::fmt::Debug for Numeric {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_str().fmt(f)
    }
}

impl std::fmt::Display for Numeric {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl From<Numeric> for u16 {
    fn from(value: Numeric) -> Self {
        value.into_int()
    }
}

impl TryFrom<u16> for Numeric {
    type Error = std::num::TryFromIntError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match Self::from_int(value) {
            Some(v) => Ok(v),
            None => Err(u8::try_from(value).unwrap_err()),
        }
    }
}

impl Numeric {
    /// Attempts to convert the provided byte slice into a `Numeric`.
    /// Returns `None` if the slice is not three digits.
    pub const fn from_bytes(bytes: &[u8]) -> Option<Numeric> {
        if bytes.len() != 3 {
            return None;
        }
        let mut retval: [u8; 3] = [0; 3];
        let mut i: usize = 0;
        while i < 3 {
            if !bytes[i].is_ascii_digit() {
                return None;
            }
            retval[i] = bytes[i];
            i += 1;
        }
        // Safety: if we left the loop, retval is 3 ASCII digits.
        Some(Numeric(unsafe { std::mem::transmute(retval) }))
    }
    /// Converts the provided byte array into a `Numeric`.
    ///
    /// # Safety
    /// The three bytes must be ASCII digits,
    /// or else undefined behavior may result from calling other functions on this type.
    pub const unsafe fn from_bytes_unchecked(bytes: [u8; 3]) -> Numeric {
        Numeric(std::mem::transmute(bytes))
    }
    /// Attempts to convert the provided integer into a `Numeric`.
    /// Returns `None` if the integer is not less than `1000`.
    pub const fn from_int(int: u16) -> Option<Numeric> {
        if int < 1000 {
            unsafe { Some(Self::from_int_unchecked(int)) }
        } else {
            None
        }
    }
    /// Converts the provided integer into a `Numeric`.
    ///
    /// # Safety
    /// The int must be less than `1000`,
    /// or else undefined behavior may result from calling other functions on this type.
    pub const unsafe fn from_int_unchecked(int: u16) -> Numeric {
        // TODO: SIMD.
        let h = (int / 100) as u8 + b'0';
        let t = ((int / 10) % 10) as u8 + b'0';
        let o = (int % 10) as u8 + b'0';
        Numeric(std::mem::transmute([h, t, o]))
    }
    /// Returns a reference to `self`'s value as a three-byte slice.
    pub const fn as_bytes(&self) -> &[u8; 3] {
        unsafe { std::mem::transmute(&self.0) }
    }
    /// Returns a reference to `self`'s value as a three-digit `str`.
    pub const fn as_str(&self) -> &str {
        unsafe { std::str::from_utf8_unchecked(self.as_bytes()) }
    }
    /// Returns `self`'s value as an unsiged integer.
    pub const fn into_int(self) -> u16 {
        // TODO: SIMD.
        let h = self.0[0].get().wrapping_sub(b'0') as u16;
        let t = self.0[1].get().wrapping_sub(b'0') as u16;
        let o = self.0[2].get().wrapping_sub(b'0') as u16;
        h * 100 + t * 10 + o
    }
    /// Writes `self` to the provided [`Write`].
    pub fn write_to(&self, write: &mut (impl Write + ?Sized)) -> std::io::Result<()> {
        write.write_all(self.as_bytes())
    }
    /// Returns `Some(true)` if `self` represents an error,
    /// `Some(false)` if it does not, or `None` if it's unknown.
    ///
    /// This function interprets the entire 000-399 range as non-errors,
    /// the 400-568 range as errors, and is defined for select numerics
    /// outside that range that are generally standard.
    pub const fn is_error(&self) -> Option<bool> {
        let num = self.into_int();
        if num < 400 {
            Some(false)
        } else if num < 569 {
            Some(true)
        } else {
            match num {
                670 => Some(false),
                671 => Some(false),
                691 => Some(true),
                696 => Some(true),
                704 => Some(false),
                705 => Some(false),
                706 => Some(false),
                723 => Some(true),
                740 => Some(false),
                741 => Some(false),
                900 => Some(false),
                901 => Some(false),
                902 => Some(true),
                903 => Some(false),
                904 => Some(true),
                905 => Some(true),
                906 => Some(true),
                907 => Some(true),
                908 => Some(false),
                _ => None,
            }
        }
    }
}
