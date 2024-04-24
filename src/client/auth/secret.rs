use crate::string::{Bytes, SecretBuf};

/// Trait for types that contain sensitive byte strings.
pub trait LoadSecret {
    /// Returns how many bytes should be preallocated for this secret.
    ///
    /// This can often be inferred without knowing the secret's value (e.g. for cryptographic keys).
    /// Defaults to 32.
    fn size_hint(&self) -> usize {
        32
    }
    /// Appends the secret's bytes to the provided buffer.
    ///
    /// This implementation may block.
    fn load_secret(self, data: &mut SecretBuf) -> std::io::Result<()>;
}

/// Trait for types that

/// Guaranteed-to-fail implementation of [`LoadSecret`].
///
/// For use when the loader in a [`Secret`] doesn't matter
/// because the secret will never actually need to be deserialized.
impl LoadSecret for () {
    fn size_hint(&self) -> usize {
        0
    }
    fn load_secret(self, _: &mut SecretBuf) -> std::io::Result<()> {
        Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "no secret loader"))
    }
}

/// A [`Deserialize`][serde::Deserialize] implementation for sensitive byte strings
/// that loads secrets at deserialization time.
///
/// Deserialization using this type may block on user input.
/// Take care when using this type in async contexts.
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Secret<T, L>(T, std::marker::PhantomData<L>);

impl<T: Clone, L> Clone for Secret<T, L> {
    fn clone(&self) -> Self {
        Secret(self.0.clone(), std::marker::PhantomData)
    }
}

impl<T: Copy, L> Copy for Secret<T, L> {}

impl<T, L> Secret<T, L> {
    /// Wraps a value, returning `self`.
    pub fn new(value: T) -> Self {
        Secret(value, std::marker::PhantomData)
    }
    /// Unwraps `self` into its contents.
    pub fn into_inner(this: Self) -> T {
        this.0
    }
}

impl<'a, T, L: LoadSecret> Secret<T, L>
where
    T: TryFrom<Bytes<'a>>,
    T::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    /// Loads the secret using the provided [`LoadSecret`] implementation.
    ///
    /// This method may block on user input.
    pub fn load(value: L) -> Result<Self, std::io::Error> {
        let mut buf = SecretBuf::with_capacity(value.size_hint());
        value.load_secret(&mut buf)?;
        let loaded = buf
            .into_bytes()
            .try_into()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        Ok(Secret::new(loaded))
    }
}

impl<T, L> std::ops::Deref for Secret<T, L> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T, L> std::ops::DerefMut for Secret<T, L> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Zero-added-security implementation of [`LoadSecret`].
///
/// This implementation stores secret data in main memory over its lifetime.
/// The value is zeroed out at the end of its lifetime.
///
/// If the `serde` and `base64` features are enabled, `Clear`
/// can be (de)serialized as a Base64-encoded string.
#[derive(Clone)]
pub struct Clear(pub SecretBuf);

impl LoadSecret for Clear {
    fn size_hint(&self) -> usize {
        0 // Don't allocate a destination buffer. We're just going to reuse the Vec.
    }
    fn load_secret(self, data: &mut SecretBuf) -> std::io::Result<()> {
        std::mem::drop(std::mem::replace(data, self.0));
        Ok(())
    }
}

#[cfg(all(feature = "serde", feature = "base64"))]
impl<'a> serde::Deserialize<'a> for Clear {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'a>,
    {
        use base64::{engine::general_purpose::STANDARD as ENGINE, Engine};
        use serde::de::Error;
        let string = String::deserialize(deserializer)?;
        let data = ENGINE.decode(string).map_err(D::Error::custom)?;
        Ok(Clear(data.into()))
    }
}

#[cfg(feature = "serde")]
impl<'a, 'b, T, S> serde::Deserialize<'a> for Secret<T, S>
where
    T: TryFrom<Bytes<'b>>,
    <T as TryFrom<Bytes<'b>>>::Error: std::fmt::Display,
    S: LoadSecret + serde::Deserialize<'a>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'a>,
    {
        use serde::de::Error;
        let loaded = S::deserialize(deserializer)?;
        let mut buf = SecretBuf::with_capacity(loaded.size_hint());
        loaded.load_secret(&mut buf).map_err(D::Error::custom)?;
        let bytes = buf.into_bytes().try_into().map_err(D::Error::custom)?;
        Ok(Secret::new(bytes))
    }
}
