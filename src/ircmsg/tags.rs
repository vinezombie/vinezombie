//! Stuctures and utilities for IRCv3 message tags.

use crate::string::{
    tf::{escape, unescape, SplitKey},
    Key, NoNul,
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
    // Might replace with a sorted Vec.
    map: BTreeMap<Key<'a>, NoNul<'a>>,
    //avail: isize,
}

impl<'a> Tags<'a> {
    /// Creates a new empty `Tags`.
    pub const fn new() -> Self {
        // avail 4094 per IRCv3 spec.
        Tags { map: BTreeMap::new() }
    }
    /// Converts `self` into a version that owns its data.
    pub fn owning(self) -> Tags<'static> {
        // This is where temptation exists to make an
        // `owning` function that takes `&mut self` and just transmute.
        Tags { map: self.map.into_iter().map(|(k, v)| (k.owning(), v.owning())).collect() }
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
    pub fn get(&self, key: impl TryInto<Key<'a>>) -> Option<&NoNul<'a>> {
        self.map.get(&key.try_into().ok()?)
    }
    /// Inserts a key with no value into this map.
    ///
    /// This is equivalent to inserting a key-value pair with an empty value.
    pub fn insert_key(&mut self, key: impl Into<Key<'a>>) -> Option<NoNul<'a>> {
        self.insert_pair(key.into(), NoNul::default())
    }
    /// Inserts a key-value pair into this map, returning the old value if present.
    pub fn insert_pair(
        &mut self,
        key: impl Into<Key<'a>>,
        value: impl Into<NoNul<'a>>,
    ) -> Option<NoNul<'a>> {
        let key = key.into();
        // TODO: Length calculations based off of the escaped size of `value`.
        self.map.insert(key, value.into())
    }
    /// Removes a key-value pair from this map, returning the value, if any.
    pub fn remove(&mut self, key: impl TryInto<Key<'a>>) -> Option<NoNul<'a>> {
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
            let (Ok(key), delim) = word.transform(SplitKey) else {
                continue;
            };
            let value = if matches!(delim, Some(b'=')) {
                let value = word.transform(Split(|b: &u8| *b == b';'));
                word.transform(SplitFirst);
                // `value` at this point is Word-valid.
                unescape(unsafe { NoNul::from_unchecked(value) })
            } else {
                NoNul::default()
            };
            tags.map.insert(key, value);
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

impl<'a> FromIterator<(Key<'a>, NoNul<'a>)> for Tags<'a> {
    fn from_iter<T: IntoIterator<Item = (Key<'a>, NoNul<'a>)>>(iter: T) -> Self {
        let mut map = Tags::new();
        for (k, v) in iter {
            map.insert_pair(k, v);
        }
        // TODO: Increase avail.
        map
    }
}

impl<'a> IntoIterator for Tags<'a> {
    type Item = (Key<'a>, NoNul<'a>);

    type IntoIter = std::collections::btree_map::IntoIter<Key<'a>, NoNul<'a>>;

    fn into_iter(self) -> Self::IntoIter {
        self.map.into_iter()
    }
}
