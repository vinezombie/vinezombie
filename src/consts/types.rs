use std::borrow::Borrow;

use crate::{
    error::ParseError,
    util::{FlatMap, FlatMapEditGuard},
};

/// Markers indicating distinct types of tags (as in discriminants in tagged unions).
///
/// This type allows conceptually treating values of [`Self::Union`] as tagged unions,
/// with tags that reperesnt values of type [`Self::Raw`].
pub trait TagClass: 'static {
    /// The type of values that [`Tag`]s in this class stand in for.
    type Raw<'a>: std::borrow::Borrow<[u8]> + Clone + Ord;
    /// The type that is treated as a tagged union, containing a tag and possibly additional data.
    type Union<'a>;
    /// Extract a shared reference to the raw tag from the outer type.
    fn get_tag<'a, 'b>(outer: &'a Self::Union<'b>) -> &'a Self::Raw<'b>;
    /// Extract a mutable reference to the raw tag type from the outer type.
    fn get_tag_mut<'a, 'b>(outer: &'a mut Self::Union<'b>) -> &'a mut Self::Raw<'b>;
}

/// Specific tag values within a [`TagClass`].
///
/// Implementors are conventionally zero-sized types.
pub trait Tag<Class: TagClass>: std::any::Any + Copy + std::fmt::Display {
    /// Returns a `'static` reference to the value this tag stands in for.
    fn as_raw(&self) -> &'static <Class as TagClass>::Raw<'static>;
}

/// [`Tag`]s that can parse the [union type][TagClass::Union] into something more useful.
pub trait TagWithValue<Class: TagClass>: Tag<Class> {
    /// The type of values associated with this tag.
    type Value<'a>;

    /// Attempt to parse this tag's value out of the union type.
    ///
    /// This function should ignore the tag in the union type and assume that it matches.
    fn from_union<'a>(
        input: &<Class as TagClass>::Union<'a>,
    ) -> Result<Self::Value<'a>, ParseError>;
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub(crate) struct TagExtractor<'a, K: TagClass, V = ()>(std::marker::PhantomData<&'a mut (K, V)>);

impl<'a, K: TagClass, V> crate::util::KeyExtractor<(K::Union<'a>, V)> for TagExtractor<'a, K, V> {
    type Key = K::Raw<'a>;
    type KeyBorrowed = [u8];

    fn extract_key<'b>(elem: &'b (K::Union<'a>, V)) -> &'b Self::Key {
        K::get_tag(&elem.0)
    }
}

// TODO: TagMap with specific value type.

/// A map of [`TagWithValue`]s in a [`TagClass`] to their respective values.
///
/// Internally, this stores the union types for the tag class and
/// parses values out of them on access.
/// It can also associate additional data with each tag.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct TagMap<K: TagClass, V: 'static = ()> {
    map: crate::util::FlatMap<(K::Union<'static>, V), TagExtractor<'static, K, V>>,
}

macro_rules! tagmap_methods {
    ($field:tt) => {
        #[doc = "Returns a shared reference to the extra value for `tag`, if any."]
        pub fn get_extra_raw(&self, tag: &K::Raw<'_>) -> Option<&V> {
            Some(&self.$field.get(tag.borrow())?.1)
        }

        #[doc = "Returns a mutable reference to the extra value for `tag`, if any."]
        pub fn get_extra_raw_mut(&mut self, tag: &K::Raw<'_>) -> Option<&mut V> {
            Some(&mut self.$field.get_mut(tag.borrow())?.1)
        }

        #[doc = "Returns a shared reference to the extra value for `tag`, if any."]
        pub fn get_extra<T: Tag<K>>(&self, tag: T) -> Option<&V> {
            self.get_extra_raw(tag.as_raw())
        }

        #[doc = "Returns a mutable reference to the extra value for `tag`, if any."]
        pub fn get_extra_mut<T: Tag<K>>(&mut self, tag: T) -> Option<&mut V> {
            self.get_extra_raw_mut(tag.as_raw())
        }

        #[doc = "Returns and parses the value stored for `tag`, if any."]
        pub fn get_parsed<T: TagWithValue<K>>(
            &self,
            tag: T,
        ) -> Option<Result<T::Value<'static>, ParseError>> {
            let (u, _) = self.$field.get(tag.as_raw().borrow())?;
            Some(T::from_union(u))
        }

        #[doc = "Returns both the parsed value and"]
        #[doc = "a shared reference to the extra value for `tag`, if any."]
        pub fn get_both<T: TagWithValue<K>>(
            &self,
            tag: T,
        ) -> Option<(Result<T::Value<'static>, ParseError>, &V)> {
            let (u, x) = self.$field.get(tag.as_raw().borrow())?;
            Some((T::from_union(u), x))
        }

        #[doc = "Returns both the parsed value and"]
        #[doc = "a mutable reference to the extra value for `tag`, if any."]
        pub fn get_both_mut<T: TagWithValue<K>>(
            &mut self,
            tag: T,
        ) -> Option<(Result<T::Value<'static>, ParseError>, &mut V)> {
            let (u, x) = self.$field.get_mut(tag.as_raw().borrow())?;
            Some((T::from_union(u), x))
        }

        #[doc = "Clears the map of all elements."]
        pub fn clear(&mut self) {
            self.$field.clear();
        }
    };
}

impl<K: TagClass, V: 'static> TagMap<K, V> {
    /// Creates a new empty map.
    pub const fn new() -> Self {
        TagMap { map: FlatMap::new() }
    }
    collection_methods!(map);
    tagmap_methods!(map);

    /// Returns a [`TagMapEditGuard`]
    pub fn edit(&mut self) -> TagMapEditGuard<'_, K, V> {
        TagMapEditGuard(self.map.edit())
    }
}

/// Edit guard for a [`TagMap`].
#[derive(Debug)]
pub struct TagMapEditGuard<'a, K: TagClass, V: 'static>(
    pub(self) FlatMapEditGuard<'a, (K::Union<'static>, V), TagExtractor<'static, K, V>>,
);

impl<'a, K: TagClass, V: 'static> TagMapEditGuard<'a, K, V> {
    collection_methods!(0);
    tagmap_methods!(0);

    /// Inserts a [union][TagClass::Union] and extra value, returning the old pair if present.
    #[inline]
    pub fn insert(&mut self, elem: K::Union<'static>, extra: V) -> Option<(K::Union<'static>, V)> {
        self.0.insert((elem, extra))
    }

    /// Inserts a [union][TagClass::Union] and extra value, or sets the extra value if the union
    /// is already present in the map.
    #[inline]
    pub fn insert_or_update(
        &mut self,
        elem: K::Union<'static>,
        extra: V,
    ) -> Option<K::Union<'static>> {
        match self.0.get_or_insert((elem, extra)) {
            (eref, Some((elem, extra))) => {
                eref.1 = extra;
                Some(elem)
            }
            _ => None,
        }
    }

    /// Inserts a [union][TagClass::Union] and extra value if not already present.
    ///
    /// Returns the arguments on *failure*.
    #[inline]
    pub fn try_insert(
        &mut self,
        elem: K::Union<'static>,
        extra: V,
    ) -> Option<(K::Union<'static>, V)> {
        self.0.try_insert((elem, extra))
    }

    /// Removes a key-value pair matching the provided `tag`, if any.
    #[inline]
    pub fn remove<T: Tag<K>>(&mut self, tag: T) -> Option<(K::Union<'static>, V)> {
        self.remove_raw(tag.as_raw())
    }

    /// Removes a key-value pair matching the provided `tag`, if any.
    #[inline]
    pub fn remove_raw(&mut self, tag: &K::Raw<'_>) -> Option<(K::Union<'static>, V)> {
        self.0.remove(tag.borrow())
    }
}

impl<K: TagClass> Default for TagMap<K> {
    fn default() -> Self {
        Self::new()
    }
}
