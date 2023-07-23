use std::ptr::NonNull;

/// Type that holds ownership of a slice.
///
/// # Safety
/// THIS TYPE DOESN'T RUN DESTRUCTORS,
pub struct OwnedSlice<T>(NonNull<T>, usize);

impl<T> OwnedSlice<T> {
    /// Converts a Vec into Self (which owns the data)
    /// and a slice with an unbound lifetime (which does not).
    ///
    /// Returns `None` and an empty slice if `value` is empty.
    ///
    /// # Safety
    /// Unbound lifetimes are the devil, and this returns a reference with one.
    pub unsafe fn from_vec<'a>(mut value: Vec<T>) -> (Option<Self>, &'a [T]) {
        if value.is_empty() {
            return (None, &[]);
        }
        // as_mut_ptr returns a dangling pointer if capacity is 0.
        // https://doc.rust-lang.org/std/vec/struct.Vec.html#method.as_mut_ptr
        let data = NonNull::new_unchecked(value.as_mut_ptr());
        let len = value.len();
        let cap = value.capacity();
        // Don't run the Vec's destructor as we're stealing ownership of its data.
        std::mem::forget(value);
        let retval = OwnedSlice(data, cap);
        (Some(retval), std::slice::from_raw_parts(data.as_ptr().cast_const(), len))
    }
}

impl<T: Clone> OwnedSlice<T> {
    /// Reconstructs a `Vec` from self using `slice`.
    ///
    /// # Safety
    /// `slice` is assumed to be a slice of the data owned by self.
    pub unsafe fn into_vec(self, slice: &[T]) -> (Vec<T>, Option<Self>) {
        let slice_start = slice.as_ptr();
        let data_start = self.0.as_ptr();
        let capacity = self.1;
        if slice_start == data_start {
            // Don't run self's destructor.
            std::mem::forget(self);
            (Vec::from_raw_parts(data_start, slice.len(), capacity), None)
        } else {
            let retval = slice.to_vec();
            (retval, Some(self))
        }
    }
}

#[cfg(feature = "zeroize")]
impl<T: zeroize::DefaultIsZeroes> OwnedSlice<T> {
    pub unsafe fn zeroize_drop(self) {
        let mut cur = self.0.as_ptr();
        let end = cur.add(self.1);
        while cur < end {
            cur.write_volatile(T::default());
            cur = cur.add(1);
        }
    }
}

impl<T> Drop for OwnedSlice<T> {
    fn drop(&mut self) {
        unsafe {
            std::mem::drop(Vec::from_raw_parts(self.0.as_ptr(), 0, self.1));
        }
    }
}
