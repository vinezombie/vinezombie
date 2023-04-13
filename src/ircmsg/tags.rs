//! Stuctures and utilities for IRCv3 message tags.

use std::{collections::BTreeMap, num::NonZeroU8};

use crate::string::{Bytes, TagKey};

/// Collection mapping tag keys to bytes.
///
/// IRCv3 requires that tag values be valid UTF-8,
/// however server implementations may be non-compliant.
/// This type can contain non-UTF-8 values,
/// but only allows insertion of [`String`]s in order to better-uphold
/// the specification's requirements.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub struct Tags<'a> {
    map: BTreeMap<TagKey<'a>, Bytes<'a>>,
    //avail: isize,
}

/// Returns the unescaped value for an escape-coded byte.
pub fn unescape(byte: &u8) -> u8 {
    match byte {
        b':' => b';',
        b's' => b' ',
        b'r' => b'\r',
        b'n' => b'\n',
        b => *b,
    }
}

/// Returns the escape code for a particular byte in a tag value,
/// or `None` if no escaping is necessary.
pub fn escape(byte: &u8) -> Option<NonZeroU8> {
    // None of the escape codes are 0u8. Take advantage of that.
    match byte {
        b';' => Some(unsafe { NonZeroU8::new_unchecked(b':') }),
        b' ' => Some(unsafe { NonZeroU8::new_unchecked(b's') }),
        b'\r' => Some(unsafe { NonZeroU8::new_unchecked(b'r') }),
        b'\n' => Some(unsafe { NonZeroU8::new_unchecked(b'n') }),
        b'\\' => Some(unsafe { NonZeroU8::new_unchecked(b'\\') }),
        _ => None,
    }
}

impl<'a> Tags<'a> {
    /// Creates a new empty `Tags`.
    pub const fn new() -> Self {
        // avail 4094 per IRCv3 spec.
        Tags { map: BTreeMap::new() }
    }
    /// Returns how many keys are in this map.
    pub fn len(&self) -> usize {
        self.map.len()
    }
    /// Returns `true` if this map contains no keys.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
    /// Inserts a key with no value into this map.
    ///
    /// This is equivalent to inserting a key-value pair with an empty value.
    pub fn insert_key(&mut self, key: impl Into<TagKey<'a>>) -> Option<Bytes<'a>> {
        self.insert_pair(key.into(), String::new())
    }
    /// Inserts a key-value pair into this map, returning the old value if present.
    pub fn insert_pair(&mut self, key: impl Into<TagKey<'a>>, value: String) -> Option<Bytes<'a>> {
        let key = key.into();
        // TODO: Length calculations based off of the escaped size of `value`.
        if let Some(v) = self.map.insert(key, value.into()) {
            // TODO: Add the old value's escaped length to avail.
            Some(v)
        } else {
            // self.avail -= key.len() as isize + self.map.is_empty() as isize;
            None
        }
    }
    /// Writes `self`, including a leading `'@'` if non-empty,
    /// to the provided [`Write`][std::io::Write].
    ///
    /// This function makes many small writes. Buffering is strongly recommended.
    pub fn write_to(&mut self, w: &mut (impl std::io::Write + ?Sized)) -> std::io::Result<()> {
        let mut prefix = b"@";
        for (key, value) in self.map.iter() {
            w.write_all(prefix)?;
            w.write_all(key.as_ref())?;
            if !value.is_empty() {
                w.write_all(b"=")?;
                w.write_all(value.as_ref())?;
            }
            prefix = b";";
        }
        Ok(())
    }
    /// Parses the provided tag string.
    ///
    /// The provided word should NOT contain the leading '@'.
    pub fn parse(word: impl Into<crate::string::Word<'a>>) -> Self {
        // TODO: Owl, the rest of it.
        Tags::new()
    }
}

/// An implementation of `Display` that includes the leading `@`.
impl std::fmt::Display for Tags<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut prefix = '@';
        for (key, value) in self.map.iter() {
            if !value.is_empty() {
                write!(f, "{prefix}{key}={value}")?;
            } else {
                write!(f, "{prefix}{key}")?;
            }
            prefix = ';';
        }
        Ok(())
    }
}

impl<'a> FromIterator<(TagKey<'a>, String)> for Tags<'a> {
    fn from_iter<T: IntoIterator<Item = (TagKey<'a>, String)>>(iter: T) -> Self {
        let mut map = Tags::new();
        for (k, v) in iter {
            map.insert_pair(k, v);
        }
        // TODO: Increase avail.
        map
    }
}

impl<'a> IntoIterator for Tags<'a> {
    type Item = (TagKey<'a>, Bytes<'a>);

    type IntoIter = std::collections::btree_map::IntoIter<TagKey<'a>, Bytes<'a>>;

    fn into_iter(self) -> Self::IntoIter {
        self.map.into_iter()
    }
}
