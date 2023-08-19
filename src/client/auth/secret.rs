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
///
/// If the `serde` and `base64` features are enabled, `Clear`
/// can be serialized as a Base64-encoded string.
#[derive(Clone)]
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

#[cfg(all(feature = "serde", feature = "base64"))]
impl serde::Serialize for Clear {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use base64::engine::{general_purpose::STANDARD as ENGINE, Engine};
        #[allow(unused_mut)]
        let mut encoded = ENGINE.encode(&self.0);
        let ok = encoded.serialize(ser)?;
        #[cfg(feature = "zeroize")]
        zeroize::Zeroize::zeroize(&mut encoded);
        Ok(ok)
    }
}

#[cfg(all(feature = "serde", feature = "base64"))]
impl<'de> serde::Deserialize<'de> for Clear {
    fn deserialize<D>(de: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use base64::engine::{general_purpose::STANDARD as ENGINE, Engine};
        use serde::de::Error;
        #[allow(unused_mut)]
        let mut encoded = String::deserialize(de)?;
        let decoded = ENGINE.decode(&encoded).map_err(D::Error::custom)?;
        #[cfg(feature = "zeroize")]
        zeroize::Zeroize::zeroize(&mut encoded);
        Ok(Clear(decoded))
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
#[cfg_attr(feature = "serde", derive(serde_derive::Serialize, serde_derive::Deserialize))]
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
