//! Implementations of specific SASL mechanisms.

use super::{LoadSecret, Sasl, SaslLogic, Secret};
use crate::string::{Arg, Bytes, NoNul};

/// The [EXTERNAL SASL mechanism](https://www.rfc-editor.org/rfc/rfc4422#appendix-A).
///
/// This is what is used when authentication occurs out-of-band,
/// such as when using TLS client certificate authentication.
///
/// The provided string, if non-empty, is an authzid.
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde_derive::Serialize, serde_derive::Deserialize))]
pub struct External(#[cfg_attr(feature = "serde", serde(default))] pub NoNul<'static>);

struct ExternalLogic(NoNul<'static>);

impl SaslLogic for ExternalLogic {
    fn reply<'a>(
        &'a mut self,
        _: &[u8],
    ) -> Result<&'a [u8], Box<dyn std::error::Error + Send + Sync + 'static>> {
        // TODO: Strictness?
        Ok(self.0.as_bytes())
    }
}

impl Sasl for External {
    fn name(&self) -> Arg<'static> {
        Arg::from_str("EXTERNAL")
    }
    fn logic(&self) -> Box<dyn SaslLogic> {
        Box::new(ExternalLogic(self.0.clone()))
    }
}

/// The [PLAIN SASL mechanism](https://www.rfc-editor.org/rfc/rfc4616).
///
/// This is what is used for typical username/password authentication.
/// It transmits the password in the clear;
/// do not use this without some form of secure transport, like TLS.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde_derive::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(bound(deserialize = "'de: 'static, S: LoadSecret + serde::de::Deserialize<'de>"))
)]
pub struct Plain<S> {
    #[cfg_attr(feature = "serde", serde(default))]
    authzid: NoNul<'static>,
    authcid: NoNul<'static>,
    passwd: Secret<Bytes<'static>, S>,
}

impl<S: LoadSecret> Plain<S> {
    /// Creates a `Plain` that logs in to the specified account.
    /// The authzid is left empty; compliant implementations should infer it.
    ///
    /// `passwd` should be UTF-8 encoded and not contain a null character,
    /// but this is not enforced.
    pub const fn new(account: NoNul<'static>, passwd: Secret<Bytes<'static>, S>) -> Self {
        Plain { authzid: NoNul::empty(), authcid: account, passwd }
    }
    /// Creates a `Plain` that logs into the account specified by authzid
    /// using the credentials for the account specified by authcid.
    ///
    /// `passwd` should be UTF-8 encoded and not contain a null character,
    /// but this is not enforced.
    pub const fn new_as(
        authzid: NoNul<'static>,
        authcid: NoNul<'static>,
        passwd: Secret<Bytes<'static>, S>,
    ) -> Self {
        Plain { authzid, authcid, passwd }
    }
}

struct PlainLogic(Bytes<'static>);

impl SaslLogic for PlainLogic {
    fn reply<'a>(
        &'a mut self,
        _: &[u8],
    ) -> Result<&'a [u8], Box<dyn std::error::Error + Send + Sync + 'static>> {
        // TODO: Strictness?
        Ok(self.0.as_bytes())
    }
}

impl<S: LoadSecret + 'static> Sasl for Plain<S> {
    fn name(&self) -> Arg<'static> {
        Arg::from_str("PLAIN")
    }
    fn logic(&self) -> Box<dyn SaslLogic> {
        use crate::string::SecretBuf;
        let authzid = self.authzid.as_bytes();
        let authcid = self.authcid.as_bytes();
        let passwd = self.passwd.as_bytes();
        // Add 2 for separating nul bytes.
        let mut data = SecretBuf::with_capacity(2 + passwd.len() + authcid.len() + authzid.len());
        data.push_cstr(authzid);
        data.push_cstr(authcid);
        data.push_slice(passwd);
        Box::new(PlainLogic(data.into_bytes()))
    }
}
