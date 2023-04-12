use crate::string::{
    ArgSafe, Bytes, KindSafe, LineSafe, NickSafe, TagKeySafe, Transform, Transformation, UserSafe,
    Utf8Policy, WordSafe,
};

/// ASCII casemapping, generic over whether it's uppercase or lowercase.
///
/// Although this transform is safe to use for
/// [`Nick`][crate::string::Nick] and [`User`][crate::string::User],
/// it is likely the result of a logic error to use this instead of [`IrcCasemap::Ascii`].
/// As such, it does not implement [`NickSafe`] or [`UserSafe`].
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub struct AsciiCasemap<const UPPERCASE: bool>;

unsafe impl<const UPPERCASE: bool> Transform for AsciiCasemap<UPPERCASE> {
    type Value<'a> = ();

    fn transform<'a>(&self, bytes: &Bytes<'a>) -> Transformation<'a, Self::Value<'a>> {
        unsafe {
            let bytes = bytes.as_slice_unsafe();
            if UPPERCASE {
                super::map_bytes(bytes, Utf8Policy::PreserveStrict, u8::to_ascii_uppercase)
            } else {
                super::map_bytes(bytes, Utf8Policy::PreserveStrict, u8::to_ascii_lowercase)
            }
        }
    }
}
unsafe impl<const UC: bool> LineSafe for AsciiCasemap<UC> {}
unsafe impl<const UC: bool> WordSafe for AsciiCasemap<UC> {}
unsafe impl<const UC: bool> TagKeySafe for AsciiCasemap<UC> {}
unsafe impl<const UC: bool> ArgSafe for AsciiCasemap<UC> {}
unsafe impl KindSafe for AsciiCasemap<true> {}

/// Basic IRC-style casemapping.
///
/// Does not map UTF-8 characters, but preserves UTF-8 validity.
/// Maps from uppercase to lowercase.
///
/// Although this transform is safe to use for [`TagKey`][crate::string::TagKey],
/// it is likely a logic error to do so (as tag keys are case-sensitive,
/// and the server's casemapping especailly does not apply to them).
/// As such, it does not implement [`TagKeySafe`].
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[non_exhaustive]
pub enum IrcCasemap {
    /// ASCII lowercase mapping.
    Ascii,
    /// ASCII casemapping, plus `[\]` are mapped to `{|}`.
    Rfc1459Strict,
    /// RFC-1459 strict casemapping, plus `~` is mapped to `^`.
    Rfc1459,
}

impl IrcCasemap {
    /// Creates a casemap from the given name.
    pub fn from_name(name: &'static [u8]) -> Option<IrcCasemap> {
        match name {
            b"ascii" => Some(IrcCasemap::Ascii),
            b"rfc1459" => Some(IrcCasemap::Rfc1459),
            b"rfc1459-strict" => Some(IrcCasemap::Rfc1459Strict),
            _ => None,
        }
    }
}

fn rfc1459_strict(byte: &u8) -> u8 {
    if matches!(byte, b'['..=b']') {
        *byte + 32
    } else {
        byte.to_ascii_lowercase()
    }
}

fn rfc1459(byte: &u8) -> u8 {
    if *byte == b'~' {
        b'^'
    } else {
        rfc1459_strict(byte)
    }
}

unsafe impl Transform for IrcCasemap {
    type Value<'a> = ();

    fn transform<'a>(&self, bytes: &Bytes<'a>) -> Transformation<'a, Self::Value<'a>> {
        use super::map_bytes;
        use Utf8Policy::PreserveStrict as U8Pol;
        unsafe {
            let bytes = bytes.as_slice_unsafe();
            match self {
                IrcCasemap::Ascii => map_bytes(bytes, U8Pol, u8::to_ascii_lowercase),
                IrcCasemap::Rfc1459Strict => map_bytes(bytes, U8Pol, rfc1459_strict),
                IrcCasemap::Rfc1459 => map_bytes(bytes, U8Pol, rfc1459),
            }
        }
    }
}
unsafe impl LineSafe for IrcCasemap {}
unsafe impl WordSafe for IrcCasemap {}
unsafe impl ArgSafe for IrcCasemap {}
unsafe impl NickSafe for IrcCasemap {}
unsafe impl UserSafe for IrcCasemap {}
