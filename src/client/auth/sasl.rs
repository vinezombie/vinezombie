//! Implementations of specific SASL mechanisms.

use super::{Sasl, SaslLogic, Secret};
use crate::string::{Arg, NoNul};

/// The [EXTERNAL SASL mechanism](https://www.rfc-editor.org/rfc/rfc4422#appendix-A).
///
/// This is what is used when authentication occurs out-of-band,
/// such as when using TLS client certificate authentication.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct External;

struct ExternalLogic;

impl SaslLogic for ExternalLogic {
    fn reply<'a>(
        &'a mut self,
        _: &[u8],
    ) -> Result<&'a [u8], Box<dyn std::error::Error + Send + Sync + 'static>> {
        // TODO: Strictness?
        Ok(Default::default())
    }
}

impl Sasl for External {
    fn name(&self) -> Arg<'static> {
        Arg::from_str("EXTERNAL")
    }
    fn logic(&self) -> std::io::Result<Box<dyn SaslLogic>> {
        Ok(Box::new(ExternalLogic))
    }
}

/// The [PLAIN SASL mechanism](https://www.rfc-editor.org/rfc/rfc4616).
///
/// This is what is used for typical username/password authentication.
/// It transmits the password in the clear;
/// do not use this without some form of secure transport, like TLS.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Plain<S> {
    authzid: NoNul<'static>,
    authcid: NoNul<'static>,
    passwd: S,
}

impl<S: Secret> Plain<S> {
    /// Creates a `Plain` that logs in to the specified account.
    /// The authzid is left empty; compliant implementations should infer it.
    ///
    /// `passwd` should be UTF-8 encoded and not contain a null character,
    /// but this is not enforced.
    pub const fn new(account: NoNul<'static>, passwd: S) -> Self {
        // TODO: NoNul::empty()
        Plain { authzid: NoNul::from_str(""), authcid: account, passwd }
    }
    /// Creates a `Plain` that logs into the account specified by authzid
    /// using the credentials for the account specified by authcid.
    ///
    /// `passwd` should be UTF-8 encoded and not contain a null character,
    /// but this is not enforced.
    pub const fn new_as(authzid: NoNul<'static>, authcid: NoNul<'static>, passwd: S) -> Self {
        Plain { authzid, authcid, passwd }
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
    fn reply<'a>(
        &'a mut self,
        _: &[u8],
    ) -> Result<&'a [u8], Box<dyn std::error::Error + Send + Sync + 'static>> {
        // TODO: Strictness?
        Ok(self.0.as_slice())
    }
}

impl<S: Secret + 'static> Sasl for Plain<S> {
    fn name(&self) -> Arg<'static> {
        Arg::from_str("PLAIN")
    }
    fn logic(&self) -> std::io::Result<Box<dyn SaslLogic>> {
        let authzid = self.authzid.as_bytes();
        let authcid = self.authcid.as_bytes();
        // 16 + 2 nul bytes.
        let mut data = Vec::with_capacity(18 + authcid.len() + authzid.len());
        data.extend(authzid);
        data.push(b'\0');
        data.extend(authcid);
        data.push(b'\0');
        self.passwd.load(&mut data)?;
        Ok(Box::new(PlainLogic(data)))
    }
}
