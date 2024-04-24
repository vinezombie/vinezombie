use std::ptr::NonNull;

/// Type that holds ownership of a slice.
///
/// # Safety
/// This type doesn't run destructors. It creates unbound lifetimes.
pub struct OwnedSlice<T>(NonNull<T>, usize);

unsafe impl<T: Send> Send for OwnedSlice<T> {}
unsafe impl<T: Sync> Sync for OwnedSlice<T> {}

impl<T> OwnedSlice<T> {
    /// Converts a Vec into Self (which owns the data)
    /// and the length of the contents.
    pub fn from_vec(mut value: Vec<T>) -> (Self, usize) {
        // as_mut_ptr returns a dangling pointer if capacity is 0.
        // https://doc.rust-lang.org/std/vec/struct.Vec.html#method.as_mut_ptr
        let data = unsafe { NonNull::new_unchecked(value.as_mut_ptr()) };
        let len = value.len();
        let cap = value.capacity();
        // Don't run the Vec's destructor as we're stealing ownership of its data.
        std::mem::forget(value);
        (OwnedSlice(data, cap), len)
    }
    /// Returns a reference to a slice of the contents.
    ///
    /// # Safety
    /// The length must be no greater than the number of initialized elements at the front
    /// of this owned slice.
    ///
    /// The included reference uses an unbound lifetime.
    /// You must either constrain this limetime or prevent safe access to it.
    pub unsafe fn as_slice<'a>(&self, len: usize) -> &'a [T] {
        std::slice::from_raw_parts(self.0.as_ptr().cast_const(), len)
    }
    pub unsafe fn into_vec_with_len(self, len: usize) -> Vec<T> {
        let ptr = self.0.as_ptr();
        let cap = self.1;
        std::mem::forget(self);
        Vec::from_raw_parts(ptr, len, cap)
    }
    /// Reconstructs a `Vec` from self using `slice`.
    ///
    /// If an OwnedSlice is returned, the elements should be considered to be uninitialized.
    ///
    /// # Safety
    /// `slice` is assumed to be a slice of the data owned by self.
    pub unsafe fn into_vec_with_slice(self, slice: &[T]) -> (Vec<T>, Option<Self>) {
        let slice_start = slice.as_ptr();
        let data_start = self.0.as_ptr();
        let capacity = self.1;
        if slice_start == data_start {
            // Don't run self's destructor.
            std::mem::forget(self);
            (Vec::from_raw_parts(data_start, slice.len(), capacity), None)
        } else {
            let mut retval = Vec::with_capacity(slice.len());
            for (src, dest) in std::iter::zip(slice, retval.spare_capacity_mut()) {
                dest.write((src as *const T).read());
            }
            retval.set_len(slice.len());
            (retval, Some(self))
        }
    }
    pub fn write_capacity(&self, len: usize) -> usize {
        self.1 - len
    }
    /// Retrieves a mutable reference to the elements AFTER `len`, as in with an index
    /// greater than or equal to `len`.
    ///
    /// # Safety
    /// It is undefined behavior to call this if any of the referenced elements are uninitialized.
    pub unsafe fn as_write_slice(&mut self, len: usize) -> &mut [T] {
        let cur = unsafe { self.0.as_ptr().add(len) };
        let len = self.1 - len;
        unsafe { std::slice::from_raw_parts_mut(cur, len) }
    }
}

impl<T: Default + Copy> OwnedSlice<T> {
    /// Sets all uninitialized values of buffer to the default value.
    pub fn init_capacity(&mut self, len: usize) {
        let mut cur = unsafe { self.0.as_ptr().add(len) };
        let end = unsafe { cur.add(self.1) };
        let default = T::default();
        while cur < end {
            unsafe {
                cur.write_volatile(default);
                cur = cur.add(1);
            }
        }
    }
    /// Overwrites the entirety of `self`'s buffer with default values.
    ///
    /// This uses `write_volatile` to keep the write from being optimized out.
    pub fn reinit_all(&mut self) {
        let mut cur = self.0.as_ptr();
        let end = unsafe { cur.add(self.1) };
        let default = T::default();
        while cur < end {
            unsafe {
                cur.write_volatile(default);
                cur = cur.add(1);
            }
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
