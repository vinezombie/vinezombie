//! Stuctures and utilities for IRCv3 message tags.

use crate::{
    string::{
        tf::{escape, unescape},
        Key, NoNul, Splitter,
    },
    util::{FlatMap, FlatMapEditGuard},
};

/// Collection mapping tag keys to bytes.
///
/// IRCv3 requires that tag values be valid UTF-8,
/// however server implementations may be non-compliant.
#[repr(transparent)]
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub struct Tags<'a> {
    pairs: FlatMap<Key<'a>, NoNul<'a>>,
}

/// Guard for editing [`Tags`].
#[derive(PartialEq, Eq, Hash, Debug)]
pub struct TagsEditGuard<'a, 'b>(FlatMapEditGuard<'b, Key<'a>, NoNul<'a>>);

impl<'a> Tags<'a> {
    /// Creates a new empty `Tags`.
    pub const fn new() -> Self {
        Tags { pairs: FlatMap::new() }
    }
    /// Converts `self` into a version that owns its data.
    pub fn owning<'b>(mut self) -> Tags<'b> {
        use crate::owning::MakeOwning;
        for (key, value) in self.pairs.as_slice_mut() {
            key.make_owning();
            value.make_owning();
        }
        unsafe { std::mem::transmute(self) }
    }
    /// Returns a guard that allows editing of `self`.
    pub fn edit(&mut self) -> TagsEditGuard<'a, '_> {
        TagsEditGuard(self.pairs.edit())
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
        self.pairs.get(key.try_into().ok()?)
    }
    /// Writes `self`, including a leading `'@'` if non-empty,
    /// to the provided [`Write`][std::io::Write].
    ///
    /// This function makes many small writes. Buffering is strongly recommended.
    pub fn write_to(&self, w: &mut (impl std::io::Write + ?Sized)) -> std::io::Result<()> {
        let mut prefix = b"@";
        for (key, value) in self.pairs.as_slice() {
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
        let word = word.into();
        if word.is_empty() {
            return Tags::new();
        }
        let mut size_hint = 1usize;
        for c in word.as_bytes() {
            size_hint += (*c == b';') as usize;
        }
        let mut splitter = Splitter::new(word);
        let mut tags = Vec::with_capacity(size_hint);
        // TODO: Tag bytes available.
        while !splitter.is_empty() {
            let Ok(key) = splitter.string::<Key>(false) else {
                splitter.consume_invalid::<Key>();
                continue;
            };
            let value = if matches!(splitter.next_byte(), Some(b'=')) {
                let value = splitter.save_end().until_byte(b';').rest::<NoNul>().unwrap();
                splitter.next_byte();
                unescape(value)
            } else {
                NoNul::default()
            };
            tags.push((key, value));
        }
        Tags { pairs: FlatMap::from_vec(tags) }
    }
}

impl<'a> TagsEditGuard<'a, '_> {
    /// Returns a mutable reference to the value associated with the provided key, if any.
    pub fn get(&mut self, key: impl TryInto<Key<'a>>) -> Option<&mut NoNul<'a>> {
        self.0.get_mut(key.try_into().ok()?)
    }
    /// Inserts a key-value pair into this map, returning the old value if present.
    pub fn insert_pair(
        &mut self,
        key: impl Into<Key<'a>>,
        value: impl Into<NoNul<'a>>,
    ) -> Option<NoNul<'a>> {
        self.0.insert(key.into(), value.into())
    }
    /// Inserts a key with no value into this map.
    ///
    /// This is equivalent to inserting a key-value pair with an empty value.
    pub fn insert_key(&mut self, key: impl Into<Key<'a>>) -> Option<NoNul<'a>> {
        self.insert_pair(key.into(), NoNul::default())
    }
    /// Removes all key-value pairs.
    pub fn clear(&mut self) {
        self.0.clear();
    }
}

/// An implementation of `Display` that includes the leading `@`.
impl std::fmt::Display for Tags<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut prefix = '@';
        for (key, value) in self.pairs.as_slice() {
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
        for (key, value) in self.pairs.as_slice() {
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
