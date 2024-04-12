use std::borrow::Borrow;

pub trait KeyExtractor<T> {
    type Key: Ord + Borrow<Self::KeyBorrowed>;
    type KeyBorrowed: Ord + ?Sized;
    fn extract_key(elem: &T) -> &Self::Key;
}

impl<K: Ord, V> KeyExtractor<(K, V)> for () {
    type Key = K;
    type KeyBorrowed = K;

    fn extract_key(elem: &(K, V)) -> &Self::Key {
        &elem.0
    }
}

/// A [`Vec`]-backed associative structure.
///
/// This is designed to have efficient lookups and batch insertion
/// at the expense of poor deletion or infrequent insertion performance.
/// It also has a key extraction type.
#[derive(Debug)]
pub struct FlatMap<E, X = ()>(Vec<E>, std::marker::PhantomData<X>);

impl<E: Clone, X> Clone for FlatMap<E, X> {
    fn clone(&self) -> Self {
        FlatMap(self.0.clone(), self.1)
    }
}

impl<E: PartialEq, X> std::cmp::PartialEq for FlatMap<E, X> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl<E: Eq, X> std::cmp::Eq for FlatMap<E, X> {}
impl<E: PartialOrd, X> std::cmp::PartialOrd for FlatMap<E, X> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.0.partial_cmp(&other.0)
    }
}
impl<E: Ord, X> std::cmp::Ord for FlatMap<E, X> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

impl<E: std::hash::Hash, X> std::hash::Hash for FlatMap<E, X> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl<E, X: KeyExtractor<E>> Default for FlatMap<E, X> {
    fn default() -> Self {
        FlatMap::new()
    }
}

/// Darkly magical guard against the inner Vec containing out-of-order elements.
///
/// This type abuses Vec::set_len to reassign the meaning of the vector's internal length variable
/// to mean not the number of initialized units of memory in the vector, but to be one past
/// the index of the last element that is part of a contiguous sequence of sorted elements
/// from the start of the Vec. The real length is kept in `real_len` which is always `>=` Vec::len.
///
/// This means that if the guard is forgotten,
/// the FlatMap will still contain only elements in sorted order.
#[derive(PartialEq, Eq, Hash, Debug)]
pub struct FlatMapEditGuard<'a, E, X: KeyExtractor<E>> {
    src: &'a mut Vec<E>,
    real_len: usize,
    marker: std::marker::PhantomData<X>,
}

impl<'a, E, X: KeyExtractor<E>> Drop for FlatMapEditGuard<'a, E, X> {
    fn drop(&mut self) {
        let sorted_until = self.src.len();
        unsafe { self.src.set_len(self.real_len) };
        if sorted_until < self.src.len() {
            self.src.sort_unstable_by(|l, r| X::extract_key(l).cmp(X::extract_key(r)));
        }
    }
}

/// Efficient keyed sequential deduplication that keeps the last duplicate element, not the first.
pub(super) fn do_dedup<E, X: KeyExtractor<E>>(vec: Vec<E>) -> Vec<E> {
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
            if X::extract_key(ptr1.as_ref().unwrap_unchecked())
                != X::extract_key(ptr2.as_ref().unwrap_unchecked())
            {
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
// TODO: We need more tests for this thing.

fn get_impl<E, X: KeyExtractor<E>>(
    pairs: &[E],
    sorted_until: usize,
    key: &X::KeyBorrowed,
) -> Option<usize> {
    // Check the sorted portion first.
    // The unsorted portion is from user additions.
    let (sorted, unsorted) = pairs.split_at(sorted_until);
    match sorted.binary_search_by(|v| X::extract_key(v).borrow().cmp(key)) {
        Ok(key) => Some(key),
        Err(v) if v < sorted_until => None,
        Err(_) => unsorted
            .iter()
            .position(|v| X::extract_key(v).borrow() == key)
            .map(|idx| idx + sorted_until),
    }
}

impl<E, X: KeyExtractor<E>> FlatMap<E, X> {
    pub const fn new() -> Self {
        FlatMap(Vec::new(), std::marker::PhantomData)
    }
    pub fn singleton(elem: E) -> Self {
        FlatMap(vec![elem], std::marker::PhantomData)
    }
    pub fn from_vec(mut vec: Vec<E>) -> Self {
        vec.sort_by(|l, r| X::extract_key(l).cmp(X::extract_key(r)));
        FlatMap(do_dedup::<E, X>(vec), std::marker::PhantomData)
    }
    pub fn edit(&mut self) -> FlatMapEditGuard<'_, E, X> {
        let real_len = self.0.len();
        FlatMapEditGuard { src: &mut self.0, real_len, marker: self.1 }
    }
    collection_methods!(0);
    pub fn get<'a>(&'a self, key: &X::KeyBorrowed) -> Option<&'a E> {
        let idx = get_impl::<E, X>(self.0.as_slice(), self.0.len(), key)?;
        Some(unsafe { self.0.get_unchecked(idx) })
    }
    pub fn get_mut<'a>(&'a mut self, key: &X::KeyBorrowed) -> Option<&'a mut E> {
        let idx = get_impl::<E, X>(self.0.as_slice(), self.0.len(), key)?;
        Some(unsafe { self.0.get_unchecked_mut(idx) })
    }
    pub fn as_slice(&self) -> &[E] {
        self.0.as_slice()
    }
    /// Returns a mutable slice. Improper use of this can violate an internal invariant
    /// that keys remain in sorted order so long as this value is not mutably borrowed.
    pub fn as_slice_mut(&mut self) -> &mut [E] {
        self.0.as_mut_slice()
    }
    pub fn clear(&mut self) {
        self.0.clear();
    }
}

impl<E, X: KeyExtractor<E>> FlatMapEditGuard<'_, E, X> {
    collection_methods!(src);

    /// Return a slice of all the elements in the `Vec`, sorted and otherwise.
    pub fn as_slice(&self) -> &[E] {
        let ptr = self.src.as_ptr();
        unsafe { std::slice::from_raw_parts(ptr, self.real_len) }
    }
    /// Return the index of a given element in the full `Vec`.
    fn get_idx(&self, key: &X::KeyBorrowed) -> Option<usize> {
        let sorted_until = self.src.len();
        get_impl::<E, X>(self.as_slice(), sorted_until, key)
    }
    pub fn get<'a>(&'a self, key: &X::KeyBorrowed) -> Option<&'a E> {
        let idx = self.get_idx(key)?;
        Some(unsafe { self.src.get_unchecked(idx) })
    }
    pub fn get_mut<'a>(&'a mut self, key: &X::KeyBorrowed) -> Option<&'a mut E> {
        let idx = self.get_idx(key)?;
        Some(unsafe { self.src.get_unchecked_mut(idx) })
    }
    fn push(&mut self, elem: E) -> &mut E {
        let key = X::extract_key(&elem);
        let mut sorted_until = self.src.len();
        let is_sorted =
            sorted_until == self.real_len && Some(key) > self.as_slice().last().map(X::extract_key);
        // Pushing to the end of a vector can panic.
        // We're going to try and ensure that can't happen
        // while the vector length is set to its true length.
        // That way, if allocation fails, at least we won't leave the FlatMap in an invalid state.
        unsafe { self.src.set_len(self.real_len) };
        match self.src.try_reserve(1) {
            Ok(()) => {
                self.src.push(elem);
                self.real_len += 1;
                sorted_until += is_sorted as usize;
            }
            Err(e) => {
                unsafe { self.src.set_len(sorted_until) };
                panic!("FlatMap failed allocation: {e}");
            }
        }
        unsafe { self.src.set_len(sorted_until) };
        let mut ptr = self.src.as_mut_ptr();
        unsafe {
            ptr = ptr.add(self.real_len);
            ptr.as_mut().unwrap_unchecked()
        }
    }
    pub fn insert(&mut self, elem: E) -> Option<E> {
        let kb = X::extract_key(&elem).borrow();
        let idx = self.get_idx(kb);
        if let Some(idx) = idx {
            let old_elem = unsafe { &mut self.src.get_unchecked_mut(idx) };
            Some(std::mem::replace(old_elem, elem))
        } else {
            self.push(elem);
            None
        }
    }
    pub fn try_insert(&mut self, elem: E) -> Option<E> {
        let kb = X::extract_key(&elem).borrow();
        if self.get_idx(kb).is_some() {
            Some(elem)
        } else {
            self.push(elem);
            None
        }
    }
    /// WARNING: Gives a mutable reference to an element.
    /// Mutation of this element can violate the ordering invariant.
    pub fn get_or_insert(&mut self, elem: E) -> (&mut E, Option<E>) {
        let kb = X::extract_key(&elem).borrow();
        let idx = self.get_idx(kb);
        if let Some(idx) = idx {
            (unsafe { self.src.get_unchecked_mut(idx) }, Some(elem))
        } else {
            (self.push(elem), None)
        }
    }
    pub fn remove(&mut self, key: &X::KeyBorrowed) -> Option<E> {
        // Given we swap_remove, this can absolutely ruin lookup performance.
        // That said, removal should be infrequent, so it's probably
        // not worth adding some sort of tombstoning to the edit guard.
        let idx = self.get_idx(key)?;
        let retval = self.src.swap_remove(idx);
        self.real_len -= 1;
        unsafe { self.src.set_len(idx) };
        Some(retval)
    }
    /// Removes all key-value pairs.
    pub fn clear(&mut self) {
        self.src.clear();
        self.real_len = 0;
    }
}

impl<E, X: KeyExtractor<E>> FromIterator<E> for FlatMap<E, X> {
    fn from_iter<T: IntoIterator<Item = E>>(iter: T) -> Self {
        FlatMap::from_vec(iter.into_iter().collect())
    }
}

// There was an Extend impl here, but it's not used by anything and needed big changes.
