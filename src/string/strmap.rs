//! String-mapping utilities.
//!
//! IRC has unusal casemapping rules that can vary from network to network,
//! so this module provides a variety of casemapping rules.
//! In addition, other forms of mapping and comparison of mapped strings are possible
//! (example: stripping prefixes, converting nicks to user cloaks).

use crate::IrcStr;

// Do not use generics to improve the API here.
// These traits must be object-safe.

/// Trait-object-safe string map.
pub trait StrMap {
    /// Returns a mapped version of the provided string.
    fn map<'a>(&self, s: IrcStr<'a>) -> IrcStr<'a>;
    /// Tests for equality post-map.
    /// The right side is assumed to already have been mapped.
    fn eq_halfmapped<'a>(&self, a: IrcStr<'a>, b: &'a str) -> bool {
        self.map(a).as_ref().eq(b)
    }
    /// Tests for equality post-map.
    fn eq<'a>(&self, a: IrcStr<'a>, b: IrcStr<'a>) -> bool {
        self.eq_halfmapped(a, self.map(b).as_ref())
    }
}

/// String map where every codepoint maps 1:1.
pub trait StrMapSimple {
    /// Returns a mapped version of the provided character.
    fn map_char(&self, c: char) -> char;
}

impl<T: StrMapSimple> StrMap for T {
    fn map<'a>(&self, s: IrcStr<'a>) -> IrcStr<'a> {
        for (idx, c) in s.char_indices() {
            if c != self.map_char(c) {
                let (mapped, unmapped) = s.as_bytes().split_at(idx);
                let (mapped, unmapped) = unsafe {
                    (std::str::from_utf8_unchecked(mapped), std::str::from_utf8_unchecked(unmapped))
                };
                let mut retval = String::with_capacity(s.as_bytes().len());
                retval.push_str(mapped);
                for c in unmapped.chars() {
                    retval.push(self.map_char(c))
                }
                return retval.into();
            }
        }
        s
    }
    fn eq_halfmapped<'a>(&self, a: IrcStr<'a>, b: &str) -> bool {
        std::iter::zip(a.chars(), b.chars()).all(|(a, b)| self.map_char(a) == b)
    }
    fn eq<'a>(&self, a: IrcStr<'a>, b: IrcStr<'a>) -> bool {
        std::iter::zip(a.chars(), b.chars()).all(|(a, b)| self.map_char(a) == self.map_char(b))
    }
}

/// Casemaps.
pub mod casemap {
    use super::StrMapSimple;
    use crate::IrcStr;
    /// Returns a casemap by name, based off of common casemap names in ISUPPORT.
    pub fn by_name(name: IrcStr<'_>) -> Option<&'static dyn StrMapSimple> {
        match name.as_ref() {
            "ascii" => Some(&Ascii),
            "rfc1459-strict" => Some(&Rfc1459Strict),
            "rfc1459" => Some(&Rfc1459),
            _ => None,
        }
    }

    /// ASCII lowercase casemap.
    #[derive(Clone, Copy, Debug)]
    pub struct Ascii;
    /// ASCII uppercase casemap.
    #[derive(Clone, Copy, Debug)]
    pub struct AsciiUpper;
    #[derive(Clone, Copy, Debug)]
    /// Stricter IRC-style casemapping.
    pub struct Rfc1459Strict;
    /// IRC-style casemapping.
    #[derive(Clone, Copy, Debug)]
    pub struct Rfc1459;

    // TODO: Other casemaps?

    impl StrMapSimple for Ascii {
        fn map_char(&self, c: char) -> char {
            c.to_ascii_lowercase()
        }
    }

    impl StrMapSimple for AsciiUpper {
        fn map_char(&self, c: char) -> char {
            c.to_ascii_uppercase()
        }
    }

    impl StrMapSimple for Rfc1459Strict {
        fn map_char(&self, c: char) -> char {
            match c {
                '[' => '{',
                '\\' => '|',
                ']' => '}',
                _ => Ascii.map_char(c),
            }
        }
    }

    impl StrMapSimple for Rfc1459 {
        fn map_char(&self, c: char) -> char {
            if c == '~' {
                '^'
            } else {
                Rfc1459Strict.map_char(c)
            }
        }
    }
}
