//! SASL mechanisms and related types.

use std::{ffi::CString, sync::Arc};

type BoxedErr = Box<dyn std::error::Error + Send + Sync>;

/// Trait for types that can store sensitive byte strings.
pub trait Secret {
    /// Appends the secret's bytes to the provided string.
    ///
    /// This method may fail if retrieving the secret fails.
    fn append_to(&self, data: &mut Vec<u8>) -> Result<(), BoxedErr>;
    /// Returns a hint for how many bytes should be preallocated for this secret.
    fn len_hint(&self) -> usize {
        0
    }
}

/// Zero-added-security implementation of `Secret`.
impl Secret for Vec<u8> {
    fn append_to(&self, data: &mut Vec<u8>) -> Result<(), BoxedErr> {
        data.extend(self.as_slice());
        Ok(())
    }
    fn len_hint(&self) -> usize {
        self.len()
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
#[derive(Clone, Copy, Default, Debug)]
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
#[derive(Clone)]
pub struct Plain<S> {
    authzid: Arc<CString>,
    authcid: Arc<CString>,
    passwd: Arc<S>,
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

impl<S> std::fmt::Debug for Plain<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Plain")
            .field("authzid", &self.authzid)
            .field("authcid", &self.authcid)
            .field("passwd", &format_args!("<?>"))
            .finish()
    }
}

struct PlainLogic(Vec<u8>);

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
        let mut data = Vec::with_capacity(self.passwd.len_hint() + authcid.len() + authzid.len());
        data.extend(authzid);
        data.extend(authcid);
        self.passwd.append_to(&mut data)?;
        Ok(Box::new(PlainLogic(data)))
    }
}

/// Enum of included SASL mechanisms and options for them.
#[derive(Clone)]
#[allow(missing_docs)]
#[non_exhaustive]
pub enum AnySasl<S> {
    External(External),
    Plain(Plain<S>),
}

impl<S> std::fmt::Debug for AnySasl<S> {
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

impl<S> From<External> for AnySasl<S> {
    fn from(value: External) -> Self {
        AnySasl::External(value)
    }
}

impl<S> From<Plain<S>> for AnySasl<S> {
    fn from(value: Plain<S>) -> Self {
        AnySasl::Plain(value)
    }
}
