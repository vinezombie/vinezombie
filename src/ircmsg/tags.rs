//! Stuctures and utilities for IRCv3 message tags.

use crate::string::{
    tf::{escape, unescape},
    Key, NoNul, Splitter,
};

/// Collection mapping tag keys to bytes.
///
/// IRCv3 requires that tag values be valid UTF-8,
/// however server implementations may be non-compliant.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub struct Tags<'a> {
    pairs: Vec<(Key<'a>, NoNul<'a>)>,
}

type KeyValuePair<'a> = (Key<'a>, NoNul<'a>);

/// Guard for editing [`Tags`].
#[derive(PartialEq, Eq, Hash, Debug)]
pub struct TagsEditGuard<'a, 'b> {
    pairs: &'b mut Vec<KeyValuePair<'a>>,
    sorted_until: usize,
}

impl<'a> Drop for TagsEditGuard<'a, '_> {
    fn drop(&mut self) {
        self.pairs.sort_unstable_by(|(k1, _), (k2, _)| k1.cmp(k2));
    }
}

fn get_impl(mut pairs: &[KeyValuePair<'_>], sorted_until: usize, key: &[u8]) -> Option<usize> {
    let mut idx = pairs.len();
    while let Some((last, rest)) = pairs.split_last() {
        idx -= 1;
        if idx < sorted_until {
            break;
        }
        pairs = rest;
        if last.0 == key {
            return Some(idx);
        }
    }
    if pairs.is_empty() {
        return None;
    }
    pairs.binary_search_by(|(k, _)| k.as_bytes().cmp(key)).ok()
}

impl<'a> Tags<'a> {
    /// Creates a new empty `Tags`.
    pub const fn new() -> Self {
        Tags { pairs: Vec::new() }
    }
    /// Converts `self` into a version that owns its data.
    pub fn owning(self) -> Tags<'static> {
        Tags { pairs: self.pairs.into_iter().map(|(k, v)| (k.owning(), v.owning())).collect() }
    }
    /// Returns a guard that allows editing of `self`.
    pub fn edit(&mut self) -> TagsEditGuard<'a, '_> {
        let sorted_until = self.pairs.len();
        TagsEditGuard { pairs: &mut self.pairs, sorted_until }
    }
    /// Returns how many keys are in this map.
    pub fn len(&self) -> usize {
        self.pairs.len()
    }
    /// Returns `true` if this map contains no keys.
    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }
    /// Returns a reference to the value associated with the provided key, if any.
    pub fn get(&self, key: impl TryInto<Key<'a>>) -> Option<&NoNul<'a>> {
        let search = key.try_into().ok()?;
        let idx = get_impl(self.pairs.as_slice(), self.pairs.len(), search.as_bytes())?;
        Some(unsafe { &self.pairs.get_unchecked(idx).1 })
    }
    /// Writes `self`, including a leading `'@'` if non-empty,
    /// to the provided [`Write`][std::io::Write].
    ///
    /// This function makes many small writes. Buffering is strongly recommended.
    pub fn write_to(&self, w: &mut (impl std::io::Write + ?Sized)) -> std::io::Result<()> {
        let mut prefix = b"@";
        for (key, value) in self.pairs.iter() {
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
        let mut tags_edit = tags.edit();
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
            tags_edit.insert_pair(key, value);
        }
        std::mem::drop(tags_edit);
        tags
    }
}

impl<'a> TagsEditGuard<'a, '_> {
    /// Returns a mutable reference to the value associated with the provided key, if any.
    pub fn get(&mut self, key: impl TryInto<Key<'a>>) -> Option<&mut NoNul<'a>> {
        let search = key.try_into().ok()?;
        let idx = get_impl(self.pairs.as_slice(), self.pairs.len(), search.as_bytes())?;
        Some(unsafe { &mut self.pairs.get_unchecked_mut(idx).1 })
    }
    /// Inserts a key-value pair into this map, returning the old value if present.
    pub fn insert_pair(
        &mut self,
        key: impl Into<Key<'a>>,
        value: impl Into<NoNul<'a>>,
    ) -> Option<NoNul<'a>> {
        let key = key.into();
        // TODO: Length calculations based off of the escaped size of `value`.
        let value = value.into();
        let idx = get_impl(self.pairs.as_slice(), self.pairs.len(), key.as_bytes());
        if let Some(idx) = idx {
            let old_value = unsafe { &mut self.pairs.get_unchecked_mut(idx).1 };
            Some(std::mem::replace(old_value, value))
        } else {
            self.pairs.push((key, value));
            None
        }
    }
    /// Inserts a key with no value into this map.
    ///
    /// This is equivalent to inserting a key-value pair with an empty value.
    pub fn insert_key(&mut self, key: impl Into<Key<'a>>) -> Option<NoNul<'a>> {
        self.insert_pair(key.into(), NoNul::default())
    }
    /// Removes all key-value pairs.
    pub fn clear(&mut self) {
        self.pairs.clear();
    }
}

/// An implementation of `Display` that includes the leading `@`.
impl std::fmt::Display for Tags<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut prefix = '@';
        for (key, value) in self.pairs.iter() {
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
        let mut tags = Tags::new();
        let mut tags_edit = tags.edit();
        for (k, v) in iter {
            tags_edit.insert_pair(k, v);
        }
        std::mem::drop(tags_edit);
        tags
    }
}

#[cfg(feature = "serde")]
impl<'a> serde::Serialize for Tags<'a> {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        let mut map = ser.serialize_map(Some(self.len()))?;
        for (key, value) in self.pairs.iter() {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}

#[cfg(feature = "serde")]
impl<'a, 'de> serde::Deserialize<'de> for Tags<'a> {
    fn deserialize<D>(de: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use std::collections::BTreeMap;
        let tags = BTreeMap::<Key<'a>, NoNul<'a>>::deserialize(de)?;
        let pairs = tags.into_iter().collect();
        Ok(Tags { pairs })
    }
}
