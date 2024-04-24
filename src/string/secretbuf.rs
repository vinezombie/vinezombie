use super::Bytes;
use crate::util::OwnedSlice;

/// A buffer containing sensitive data.
pub struct SecretBuf {
    data: std::mem::ManuallyDrop<OwnedSlice<u8>>,
    len: usize,
}

impl SecretBuf {
    /// Creates a new `SecretBuf` with space for at least `count` bytes.
    pub fn with_capacity(count: usize) -> SecretBuf {
        let vec = Vec::with_capacity(count);
        let (mut data, len) = OwnedSlice::from_vec(vec);
        data.init_capacity(len);
        let data = std::mem::ManuallyDrop::new(data);
        SecretBuf { data, len }
    }
    /// Allows read-only access to the contents of this buffer through a [`Bytes`].
    pub fn as_bytes(&self) -> Bytes<'_> {
        Bytes::from(unsafe { self.data.as_slice(self.len) }).secret()
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
}

impl From<Vec<u8>> for SecretBuf {
    fn from(value: Vec<u8>) -> Self {
        let (mut data, len) = OwnedSlice::from_vec(value);
        data.init_capacity(len);
        let data = std::mem::ManuallyDrop::new(data);
        SecretBuf { data, len }
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
