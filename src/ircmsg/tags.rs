//! Stuctures and utilities for IRCv3 message tags.

use crate::string::{
    tf::{escape, unescape},
    Key, NoNul, Splitter,
};
use std::collections::BTreeMap;

/// Collection mapping tag keys to bytes.
///
/// IRCv3 requires that tag values be valid UTF-8,
/// however server implementations may be non-compliant.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub struct Tags<'a>(BTreeMap<Key<'a>, NoNul<'a>>);

impl<'a> Tags<'a> {
    /// Creates a new empty `Tags`.
    pub const fn new() -> Self {
        Tags(BTreeMap::new())
    }
    /// Converts `self` into a version that owns its data.
    pub fn owning(self) -> Tags<'static> {
        // This is where temptation exists to make an
        // `owning` function that takes `&mut self` and just transmute.
        Tags(self.0.into_iter().map(|(k, v)| (k.owning(), v.owning())).collect())
    }
    /// Returns how many keys are in this map.
    pub fn len(&self) -> usize {
        self.0.len()
    }
    /// Returns `true` if this map contains no keys.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    /// Returns a reference to the value associated with the provided key, if any.
    pub fn get(&self, key: impl TryInto<Key<'a>>) -> Option<&NoNul<'a>> {
        self.0.get(&key.try_into().ok()?)
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
        self.0.insert(key, value.into())
    }
    /// Removes a key-value pair from this map, returning the value, if any.
    pub fn remove(&mut self, key: impl TryInto<Key<'a>>) -> Option<NoNul<'a>> {
        let key = key.try_into().ok()?;
        self.0.remove(&key)
    }
    /// Removes all key-value pairs.
    pub fn clear(&mut self) {
        self.0.clear();
    }
    /// Writes `self`, including a leading `'@'` if non-empty,
    /// to the provided [`Write`][std::io::Write].
    ///
    /// This function makes many small writes. Buffering is strongly recommended.
    pub fn write_to(&self, w: &mut (impl std::io::Write + ?Sized)) -> std::io::Result<()> {
        let mut prefix = b"@";
        for (key, value) in self.0.iter() {
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
        let mut splitter = Splitter::new(word.into());
        let mut tags = Tags::new();
        // TODO: Tag bytes available.
        while !splitter.is_empty() {
            let Ok(key) = splitter.string::<Key>(false) else {
                continue;
            };
            let value = if matches!(splitter.next_byte(), Some(b'=')) {
                let value = splitter.save_end().until_byte(b';').rest::<NoNul>().unwrap();
                splitter.next_byte();
                unescape(value)
            } else {
                NoNul::default()
            };
            tags.0.insert(key, value);
        }
        tags
    }
}

/// An implementation of `Display` that includes the leading `@`.
impl std::fmt::Display for Tags<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut prefix = '@';
        for (key, value) in self.0.iter() {
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
        self.0.into_iter()
    }
}
