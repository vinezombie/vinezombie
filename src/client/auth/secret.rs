use std::sync::Arc;

/// Trait for types that contain sensitive byte strings.
pub trait Secret {
    /// Appends the secret's bytes to the provided buffer.
    fn load(&self, data: &mut Vec<u8>) -> std::io::Result<()>;
    /// Creates a new secret with the provided bytes.
    fn new(data: Vec<u8>) -> std::io::Result<Self>
    where
        Self: Sized;
}

/// Trait for types that

/// Guaranteed-to-fail implementation of [`Secret`].
///
/// For use when the type used in a `Secret`-bounded type parameter doesn't matter
/// because the secret will never actually be used.
impl Secret for () {
    fn load(&self, _: &mut Vec<u8>) -> std::io::Result<()> {
        Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "no secret"))
    }

    fn new(_data: Vec<u8>) -> std::io::Result<Self> {
        #[cfg(feature = "zeroize")]
        std::mem::drop(zeroize::Zeroizing::new(_data));
        Ok(())
    }
}

/// Zero-added-security implementation of [`Secret`].
///
/// This implementation stores secret data in main memory over its lifetime.
/// If the `zeroize` feature is enabled, the underlying vector is zeroed out
/// at the end of its lifetime.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Clear(pub Vec<u8>);

impl Secret for Clear {
    fn load(&self, data: &mut Vec<u8>) -> std::io::Result<()> {
        let lens = data.len() + self.0.len();
        if data.capacity() < lens {
            let mut vec = Vec::with_capacity(lens);
            vec.extend_from_slice(data.as_slice());
            #[cfg(feature = "zeroize")]
            zeroize::Zeroize::zeroize(data);
            *data = vec;
        }
        data.extend_from_slice(self.0.as_slice());
        Ok(())
    }

    fn new(data: Vec<u8>) -> std::io::Result<Self> {
        Ok(Clear(data))
    }
}

impl Drop for Clear {
    fn drop(&mut self) {
        #[cfg(feature = "zeroize")]
        zeroize::Zeroize::zeroize(&mut self.0);
    }
}

/// Shared container for [`Secret`] impls.
///
/// This [`Arc`] newtype has a `Debug` impl that always prints `<?>`.
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SharedSecret<S: Secret>(Arc<S>);

impl Default for SharedSecret<()> {
    fn default() -> Self {
        SharedSecret(Arc::new(()))
    }
}

impl<S: Secret> std::fmt::Debug for SharedSecret<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        crate::string::DISPLAY_PLACEHOLDER.fmt(f)
    }
}

impl<S: Secret> Secret for SharedSecret<S> {
    fn load(&self, data: &mut Vec<u8>) -> std::io::Result<()> {
        self.0.load(data)
    }

    fn new(data: Vec<u8>) -> std::io::Result<Self> {
        Ok(Self(Arc::new(S::new(data)?)))
    }
}
