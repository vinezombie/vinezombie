//! SASL mechanisms and related types.

use std::{ffi::CString, sync::Arc};

type BoxedErr = Box<dyn std::error::Error + Send + Sync>;

/// Trait for types that can store sensitive byte strings.
pub trait Secret {
    /// Appends the secret's bytes to the provided buffer.
    fn load(&self, data: &mut Vec<u8>) -> Result<(), BoxedErr>;
    /// Sets this secret's value.
    fn store(&mut self, data: Vec<u8>) -> Result<(), BoxedErr>;
    /// Irreversibly destroys any sensitive data owned by self.
    ///
    /// The actions performed by this method should ideally be performed by [`Drop`] impl,
    /// however this method
    fn destroy(&mut self) {}
}

/// Zero-added-security implementation of [`Secret`].
impl Secret for Vec<u8> {
    fn load(&self, data: &mut Vec<u8>) -> Result<(), BoxedErr> {
        let lens = data.len() + self.len();
        if data.capacity() < lens {
            let mut vec = Vec::with_capacity(lens);
            vec.extend_from_slice(data.as_slice());
            #[cfg(feature = "zeroize")]
            zeroize::Zeroize::zeroize(data);
            *data = vec;
        }
        data.extend_from_slice(self.as_slice());
        Ok(())
    }

    fn store(&mut self, data: Vec<u8>) -> Result<(), BoxedErr> {
        self.destroy();
        *self = data;
        Ok(())
    }

    fn destroy(&mut self) {
        #[cfg(feature = "zeroize")]
        zeroize::Zeroize::zeroize(self);
    }
}

/// The logic of a SASL mechanism.
pub trait SaslLogic {
    /// Handles data sent by the server.
    fn reply<'a>(&'a mut self, data: &[u8]) -> Result<&'a [u8], BoxedErr>;
}

/// SASL mechanisms.
pub trait Sasl: std::fmt::Debug {
    /// The name of this mechanism, as the client requests it.
    fn name(&self) -> &'static str;
    /// Returns the logic for this mechanism as a [`SaslLogic]`.
    fn logic(&self) -> Result<Box<dyn SaslLogic>, BoxedErr>;
}

/// The [EXTERNAL SASL mechanism](https://www.rfc-editor.org/rfc/rfc4422#appendix-A).
///
/// This is what is used when authentication occurs out-of-band,
/// such as when using TLS client certificate authentication.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct External;

struct ExternalLogic;

impl SaslLogic for ExternalLogic {
    fn reply<'a>(&'a mut self, _: &[u8]) -> Result<&'a [u8], BoxedErr> {
        // TODO: Strictness?
        Ok(Default::default())
    }
}

impl Sasl for External {
    fn name(&self) -> &'static str {
        "EXTERNAL"
    }
    fn logic(&self) -> Result<Box<dyn SaslLogic>, BoxedErr> {
        Ok(Box::new(ExternalLogic))
    }
}

/// The [PLAIN SASL mechanism](https://www.rfc-editor.org/rfc/rfc4616).
///
/// This is what is used for typical username/password authentication.
/// It transmits the password in the clear;
/// do not use this without some form of secure transport, like TLS.
#[derive(Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Plain<S: Secret> {
    authzid: Arc<CString>,
    authcid: Arc<CString>,
    passwd: Arc<S>,
}

impl<S: Secret> Drop for Plain<S> {
    fn drop(&mut self) {
        if let Some(passwd) = Arc::get_mut(&mut self.passwd) {
            passwd.destroy();
        }
    }
}

impl<S: Secret> Plain<S> {
    /// Creates a `Plain` that logs in to the specified account.
    /// The authzid is left empty; compliant implementations should infer it.
    ///
    /// `passwd` should be UTF-8 encoded and not contain a null character,
    /// but this is not enforced.
    pub fn new(account: CString, passwd: S) -> Self {
        Plain {
            authzid: Arc::new(CString::new("").unwrap()),
            authcid: Arc::new(account),
            passwd: Arc::new(passwd),
        }
    }
    /// Creates a `Plain` that logs into the account specified by authzid
    /// using the credentials for the account specified by authcid.
    ///
    /// `passwd` should be UTF-8 encoded and not contain a null character,
    /// but this is not enforced.
    pub fn new_as(authzid: CString, authcid: CString, passwd: S) -> Self {
        Plain { authzid: Arc::new(authzid), authcid: Arc::new(authcid), passwd: Arc::new(passwd) }
    }
}

impl<S: Secret> std::fmt::Debug for Plain<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Plain")
            .field("authzid", &self.authzid)
            .field("authcid", &self.authcid)
            .field("passwd", &crate::string::DISPLAY_PLACEHOLDER)
            .finish()
    }
}

struct PlainLogic(Vec<u8>);

#[cfg(feature = "zeroize")]
impl Drop for PlainLogic {
    fn drop(&mut self) {
        zeroize::Zeroize::zeroize(&mut self.0);
    }
}

impl SaslLogic for PlainLogic {
    fn reply<'a>(&'a mut self, _: &[u8]) -> Result<&'a [u8], BoxedErr> {
        // TODO: Strictness?
        Ok(self.0.as_slice())
    }
}

impl<S: Secret + 'static> Sasl for Plain<S> {
    fn name(&self) -> &'static str {
        "PLAIN"
    }
    fn logic(&self) -> Result<Box<dyn SaslLogic>, BoxedErr> {
        let authzid = self.authzid.as_bytes_with_nul();
        let authcid = self.authcid.as_bytes_with_nul();
        let mut data = Vec::with_capacity(16 + authcid.len() + authzid.len());
        data.extend(authzid);
        data.extend(authcid);
        self.passwd.load(&mut data)?;
        Ok(Box::new(PlainLogic(data)))
    }
}

/// Enum of included SASL mechanisms and options for them.
#[derive(Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
#[non_exhaustive]
pub enum AnySasl<S: Secret> {
    External(External),
    Plain(Plain<S>),
}

impl<S: Secret> std::fmt::Debug for AnySasl<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnySasl::External(s) => s.fmt(f),
            AnySasl::Plain(s) => s.fmt(f),
        }
    }
}

impl<S: Secret + 'static> Sasl for AnySasl<S> {
    fn name(&self) -> &'static str {
        match self {
            AnySasl::External(s) => s.name(),
            AnySasl::Plain(s) => s.name(),
        }
    }

    fn logic(&self) -> Result<Box<dyn SaslLogic>, BoxedErr> {
        match self {
            AnySasl::External(s) => s.logic(),
            AnySasl::Plain(s) => s.logic(),
        }
    }
}

impl<S: Secret> From<External> for AnySasl<S> {
    fn from(value: External) -> Self {
        AnySasl::External(value)
    }
}

impl<S: Secret> From<Plain<S>> for AnySasl<S> {
    fn from(value: Plain<S>) -> Self {
        AnySasl::Plain(value)
    }
}
