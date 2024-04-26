use super::Bytes;
use crate::util::OwnedSlice;

/// A buffer containing sensitive data.
pub struct SecretBuf {
    data: std::mem::ManuallyDrop<OwnedSlice<u8>>,
    len: usize,
}

impl SecretBuf {
    /// Returns `true` if this buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    /// Creates a new `SecretBuf` with space for at least `count` bytes.
    pub fn with_capacity(count: usize) -> Self {
        let vec = Vec::with_capacity(count);
        let (mut data, len) = OwnedSlice::from_vec(vec);
        data.init_capacity(len);
        let data = std::mem::ManuallyDrop::new(data);
        SecretBuf { data, len }
    }
    /// Allows read-only access to the contents of this buffer through a [`Bytes`].
    pub fn as_bytes(&self) -> Bytes<'_> {
        Bytes::from(self.as_ref()).secret()
    }
    /// Effeciently converts `self` into a secret [`Bytes`].
    pub fn into_bytes<'a>(mut self) -> Bytes<'a> {
        let data = unsafe { std::mem::ManuallyDrop::take(&mut self.data) };
        let len = self.len;
        std::mem::forget(self);
        Bytes::from_secret(unsafe { data.into_vec_with_len(len) })
    }
    /// Reserves enough length for `len` more bytes.
    pub fn reserve(&mut self, len: usize) {
        if self.data.write_capacity(self.len) < len {
            let mut resized = std::mem::ManuallyDrop::new(unsafe {
                let mut vec = Vec::with_capacity(self.len + len);
                vec.extend_from_slice(self.data.as_slice(self.len));
                OwnedSlice::from_vec(vec).0
            });
            resized.init_capacity(self.len);
            std::mem::swap(&mut self.data, &mut resized);
            resized.reinit_all();
            std::mem::ManuallyDrop::into_inner(resized);
        }
    }
    /// Appends the provided byte.
    /// If this buffer needs to reallocate, allocates `extra` additional bytes.
    pub fn push(&mut self, byte: u8, extra: usize) {
        if self.len == self.data.capacity() {
            self.reserve(1 + extra);
        }
        let wref = unsafe { self.data.as_write_slice(self.len).first_mut().unwrap_unchecked() };
        *wref = byte;
        self.len += 1;
    }
    /// Push the provided bytes and an additional null byte.
    ///
    /// The null byte is used as a field separator for some SASL mechanisms.
    pub fn push_cstr(&mut self, slice: &[u8]) {
        let len = slice.len() + 1;
        self.reserve(len);
        let (last, write_slice) =
            unsafe { &mut self.data.as_write_slice(self.len)[..len] }.split_last_mut().unwrap();
        write_slice.copy_from_slice(slice);
        *last = b'\0';
        self.len += len;
    }
    /// Appends the provided slice to this buffer.
    pub fn push_slice(&mut self, slice: &[u8]) {
        let len = slice.len();
        let _ = self.read_from(len, &mut std::io::Cursor::new(slice));
    }
    /// Reads up to `expect` bytes from the provided [`Read`][std::io::Read]er
    /// into this buffer.
    pub fn read_from(
        &mut self,
        expect: usize,
        read: &mut (impl std::io::Read + ?Sized),
    ) -> Result<usize, std::io::Error> {
        self.reserve(expect);
        // Safety: This data gets initialized on creation of the SecretBuf,
        // or above.
        let mut write_to = unsafe { self.data.as_write_slice(self.len) };
        let mut bytes_read = 0usize;
        while bytes_read < expect {
            let last_read = read.read(write_to)?;
            if last_read == 0 {
                break;
            }
            write_to = write_to.split_at_mut(last_read).1;
            bytes_read += last_read;
        }
        // Only increment this after everything has been read to avoid partial reads.
        self.len += bytes_read;
        Ok(bytes_read)
    }
    /// Clears the contents of `self`.
    ///
    /// Does not zero out the buffer.
    pub fn clear(&mut self) {
        self.len = 0;
    }
}

/// This is implemented in order to allow `std::mem::take` and relatives to work.
/// For most usecases, it is strongly recommmended to use [`SecretBuf::with_capacity`] instead
/// as reallocations are more expensive with this type due to needing to zero the buffer each time.
impl Default for SecretBuf {
    fn default() -> Self {
        Self::with_capacity(0)
    }
}

impl From<Vec<u8>> for SecretBuf {
    fn from(value: Vec<u8>) -> Self {
        let (mut data, len) = OwnedSlice::from_vec(value);
        data.init_capacity(len);
        let data = std::mem::ManuallyDrop::new(data);
        SecretBuf { data, len }
    }
}

impl FromIterator<u8> for SecretBuf {
    fn from_iter<T: IntoIterator<Item = u8>>(iter: T) -> Self {
        let mut iter = iter.into_iter();
        let size = match iter.size_hint() {
            (min, Some(max)) => std::cmp::max(min, std::cmp::min(u16::MAX as usize, max)),
            (min, None) => min,
        };
        let mut retval = Self::with_capacity(size);
        while let Some(b) = iter.next() {
            // Performance can degrade severely here for very long iterators.
            // Hopefully nobody tries to make a secret that's 66k bytes long
            // from a filtered iterator.
            retval.push(b, iter.size_hint().0);
        }
        retval
    }
}

impl Clone for SecretBuf {
    fn clone(&self) -> Self {
        SecretBuf::from(self.as_bytes().to_vec())
    }
}

impl Drop for SecretBuf {
    fn drop(&mut self) {
        self.data.reinit_all();
        unsafe { std::mem::ManuallyDrop::drop(&mut self.data) }
    }
}

impl AsRef<[u8]> for SecretBuf {
    fn as_ref(&self) -> &[u8] {
        unsafe { self.data.as_slice(self.len) }
    }
}
