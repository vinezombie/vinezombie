//! Stuctures and utilities for IRCv3 message tags.

use crate::string::{
    tf::{escape, unescape},
    Bytes, TagKey,
};
use std::collections::BTreeMap;

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
    /// Returns a reference to the value associated with the provided key, if any.
    pub fn get(&self, key: impl TryInto<TagKey<'a>>) -> Option<&Bytes<'a>> {
        self.map.get(&key.try_into().ok()?)
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
        self.map.insert(key, value.into())
    }
    /// Removes a key-value pair from this map, returning the value, if any.
    pub fn remove(&mut self, key: impl TryInto<TagKey<'a>>) -> Option<Bytes<'a>> {
        let key = key.try_into().ok()?;
        self.map.remove(&key)
    }
    /// Removes all key-value pairs.
    pub fn clear(&mut self) {
        self.map.clear();
    }
    /// Writes `self`, including a leading `'@'` if non-empty,
    /// to the provided [`Write`][std::io::Write].
    ///
    /// This function makes many small writes. Buffering is strongly recommended.
    pub fn write_to(&self, w: &mut (impl std::io::Write + ?Sized)) -> std::io::Result<()> {
        let mut prefix = b"@";
        for (key, value) in self.map.iter() {
            w.write_all(prefix)?;
            w.write_all(key.as_ref())?;
            if !value.is_empty() {
                w.write_all(b"=")?;
                w.write_all(escape(value.clone()).as_ref())?;
            }
            prefix = b";";
        }
        Ok(())
    }
    /// Parses the provided semicolon-delimited list of tag strings.
    ///
    /// The provided word should NOT contain the leading '@'.
    pub fn parse(word: impl Into<crate::string::Word<'a>>) -> Self {
        use crate::string::tf::{Split, SplitFirst};
        let mut word = word.into();
        let mut tags = Tags::new();
        // TODO: Tag bytes available.
        while !word.is_empty() {
            let key = word.transform(Split(crate::string::is_invalid_for_tagkey::<false>));
            let value = if matches!(word.transform(SplitFirst), Some(b'=')) {
                let value = word.transform(Split(|b: &u8| *b == b';'));
                word.transform(SplitFirst);
                value
            } else {
                Bytes::empty()
            };
            if key.is_empty() {
                continue;
            }
            let key = unsafe { TagKey::from_unchecked(key) };
            tags.map.insert(key, unescape(value));
        }
        tags
    }
}

/// An implementation of `Display` that includes the leading `@`.
impl std::fmt::Display for Tags<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut prefix = '@';
        for (key, value) in self.map.iter() {
            if !value.is_empty() {
                let value = escape(value.clone());
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
