use std::borrow::Borrow;

/// A [`Vec`]-backed associative structure.
///
/// This is designed to have efficient lookups and batch insertion
/// at the expense of poor deletion or infrequent insertion performance.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct FlatMap<K, V>(Vec<(K, V)>);

impl<K: Ord, V> Default for FlatMap<K, V> {
    fn default() -> Self {
        FlatMap::new()
    }
}

#[derive(PartialEq, Eq, Hash, Debug)]
pub struct FlatMapEditGuard<'a, K: Ord, V> {
    pairs: &'a mut Vec<(K, V)>,
    sorted_until: usize,
}

impl<'a, K: Ord, V> Drop for FlatMapEditGuard<'a, K, V> {
    fn drop(&mut self) {
        if self.sorted_until < self.pairs.len() {
            self.pairs.sort_unstable_by(|(k1, _), (k2, _)| k1.cmp(k2));
        }
    }
}

/// Efficient keyed sequential deduplication that keeps the last duplicate element, not the first.
fn do_dedup<K: Eq, V>(vec: Vec<(K, V)>) -> Vec<(K, V)> {
    // What follows uses a lot of unsafe.
    if vec.len() < 2 {
        return vec;
    }
    let (ptr, len, cap) = {
        // This is what std does for the currently-unstable into_raw_parts.
        let mut me = std::mem::ManuallyDrop::new(vec);
        (me.as_mut_ptr(), me.len(), me.capacity())
    };
    // `last` is the pointer to the last element, NOT one past the end.
    let (mut ptr1, mut ptr2, last) = (ptr, ptr, unsafe { ptr.add(len - 1) });
    while ptr2 < last {
        // If true, the elements in the range (ptr1, ptr2] are uninitialized,
        // and we'll need to move from ptr2 post-incement to move the hole.
        // We can test here because ptr1 keeps up with ptr2
        // if and only if the keys were never equal.
        let hole = ptr1 != ptr2;
        unsafe {
            ptr2 = ptr2.add(1);
            // Compare keys.
            if ptr1.as_ref().unwrap_unchecked().0 != ptr2.as_ref().unwrap_unchecked().0 {
                ptr1 = ptr1.add(1);
                if hole {
                    std::ptr::copy_nonoverlapping(ptr2, ptr1, 1);
                    // Drop nothing. Just consider the data at ptr2 uninitialized.
                }
            } else {
                ptr1.drop_in_place();
                std::ptr::copy_nonoverlapping(ptr2, ptr1, 1);
            }
        }
    }
    unsafe {
        // +1 because ptr1 is not one past the end.
        let new_len = 1 + ptr1.offset_from(ptr) as usize;
        Vec::from_raw_parts(ptr, new_len, cap)
    }
}

#[test]
fn test_dedup() {
    let testcases = [
        (vec![], [].as_slice()),
        (vec![(1, 0)], &[(1, 0)]),
        (vec![(1, 0), (2, 0)], &[(1, 0), (2, 0)]),
        (vec![(1, 0), (2, 0), (3, 0)], &[(1, 0), (2, 0), (3, 0)]),
        (vec![(1, 0), (2, 0), (2, 0)], &[(1, 0), (2, 0)]),
        (vec![(1, 0), (1, 0), (2, 0)], &[(1, 0), (2, 0)]),
        (vec![(1, 0), (2, 0), (2, 0), (3, 0)], &[(1, 0), (2, 0), (3, 0)]),
        (vec![(1, 0), (2, 0), (2, 1), (2, 2), (3, 0)], &[(1, 0), (2, 2), (3, 0)]),
    ];
    for (init, expected) in testcases {
        let result = do_dedup(init);
        assert_eq!(&result, expected);
    }
    // Simple test to hopefully catch UAFs.
    let vec1 = do_dedup(vec![
        (1, String::from("foo")),
        (1, String::from("bar")),
        (2, String::from("baz")),
    ]);
    let vec2 = do_dedup(vec![
        (1, String::from("bar")),
        (2, String::from("foo")),
        (2, String::from("baz")),
    ]);
    assert_eq!(vec1, vec2);
}

// TODO: We need more tests for this thing.

fn get_impl<K: Ord, V>(pairs: &[(K, V)], sorted_until: usize, key: &K) -> Option<usize> {
    // Check the sorted portion first.
    // The unsorted portion is from user additions.
    let (sorted, unsorted) = pairs.split_at(sorted_until);
    match sorted.binary_search_by(|(k, _)| k.cmp(key)) {
        Ok(key) => Some(key),
        Err(v) if v < sorted_until => None,
        Err(_) => unsorted.iter().position(|(k, _)| k == key).map(|idx| idx + sorted_until),
    }
}

impl<K: Ord, V> FlatMap<K, V> {
    pub const fn new() -> Self {
        FlatMap(Vec::new())
    }
    pub fn from_vec(mut vec: Vec<(K, V)>) -> Self {
        vec.sort_by(|(k1, _), (k2, _)| k1.cmp(k2));
        FlatMap(do_dedup(vec))
    }
    pub fn edit(&mut self) -> FlatMapEditGuard<'_, K, V> {
        let sorted_until = self.0.len();
        FlatMapEditGuard { pairs: &mut self.0, sorted_until }
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn get(&self, key: impl Borrow<K>) -> Option<&V> {
        let idx = get_impl(self.0.as_slice(), self.0.len(), key.borrow())?;
        Some(unsafe { &self.0.get_unchecked(idx).1 })
    }
    pub fn get_mut(&mut self, key: impl Borrow<K>) -> Option<&mut V> {
        let idx = get_impl(self.0.as_slice(), self.0.len(), key.borrow())?;
        Some(unsafe { &mut self.0.get_unchecked_mut(idx).1 })
    }
    pub fn as_slice(&self) -> &[(K, V)] {
        self.0.as_slice()
    }
    /// Returns a mutable slice. Improper use of this can violate an internal invariant
    /// that keys remain in sorted order so long as this value is not mutably borrowed.
    pub fn as_slice_mut(&mut self) -> &mut [(K, V)] {
        self.0.as_mut_slice()
    }
}

impl<K: Ord, V> FlatMapEditGuard<'_, K, V> {
    pub fn get(&self, key: impl Borrow<K>) -> Option<&V> {
        let idx = get_impl(self.pairs.as_slice(), self.sorted_until, key.borrow())?;
        Some(unsafe { &self.pairs.get_unchecked(idx).1 })
    }
    pub fn get_mut(&mut self, key: impl Borrow<K>) -> Option<&mut V> {
        let idx = get_impl(self.pairs.as_slice(), self.sorted_until, key.borrow())?;
        Some(unsafe { &mut self.pairs.get_unchecked_mut(idx).1 })
    }
    fn push(&mut self, key: K, value: V) {
        if self.sorted_until == self.pairs.len() && Some(&key) > self.pairs.last().map(|k| &k.0) {
            self.sorted_until += 1;
        }
        self.pairs.push((key, value));
    }
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        let idx = get_impl(self.pairs.as_slice(), self.sorted_until, key.borrow());
        if let Some(idx) = idx {
            let old_value = unsafe { &mut self.pairs.get_unchecked_mut(idx).1 };
            Some(std::mem::replace(old_value, value))
        } else {
            self.push(key, value);
            None
        }
    }
    pub fn remove(&mut self, key: impl Borrow<K>) -> Option<V> {
        // Given we swap_remove, this can absolutely ruin lookup performance.
        // That said, removal should be infrequent, so it's probably
        // not worth adding some sort of tombstoning to the edit guard.
        let idx = get_impl(self.pairs.as_slice(), self.sorted_until, key.borrow())?;
        self.sorted_until = idx;
        Some(self.pairs.swap_remove(idx).1)
    }
    /// Removes all key-value pairs.
    pub fn clear(&mut self) {
        self.pairs.clear();
        self.sorted_until = 0;
    }
}

impl<K: Ord, V> FromIterator<(K, V)> for FlatMap<K, V> {
    fn from_iter<T: IntoIterator<Item = (K, V)>>(iter: T) -> Self {
        FlatMap::from_vec(iter.into_iter().collect())
    }
}

impl<K: Ord, V> Extend<(K, V)> for FlatMapEditGuard<'_, K, V> {
    fn extend<T: IntoIterator<Item = (K, V)>>(&mut self, iter: T) {
        let iter = iter.into_iter();
        self.pairs.reserve(iter.size_hint().0);
        for (key, value) in iter {
            self.insert(key, value);
        }
    }

    // Unstable.
    /*
    fn extend_one(&mut self, item: (K, V)) {
        let (key, value) = item;
        self.insert(item, value);
    }

    fn extend_reserve(&mut self, additional: usize) {
        self.pairs.reserve(additional)
    }
    */
}
