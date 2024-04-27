use crate::{
    client::auth::{LoadSecret, Sasl, SaslLogic, Secret},
    string::{Arg, NoNul, SecretBuf},
};
use std::collections::BTreeSet;

static SASL_PLAIN_NAME: Arg = Arg::from_str("PLAIN");

/// The set of mechanisms supported by [`Password`].
#[non_exhaustive]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde_derive::Deserialize))]
pub enum PasswordMechanism {
    /// The [PLAIN](https://datatracker.ietf.org/doc/html/rfc4616) mechanism.
    #[default]
    Plain,
    // Commenting this out for now pending an actual implementation of SCRAM.
    // #[cfg(feature = "crypto")]
    // /// The [SCRAM](https://datatracker.ietf.org/doc/html/rfc5802) mechanism with SHA-256.
    // ScramSha256,
    // #[cfg(feature = "crypto")]
    // /// The [SCRAM](https://datatracker.ietf.org/doc/html/rfc5802) mechanism with SHA-512.
    // ScramSha512,
}

impl PasswordMechanism {
    pub(self) fn full_set() -> BTreeSet<PasswordMechanism> {
        [PasswordMechanism::Plain].into_iter().collect()
    }
    pub(self) fn logic(&self, authzid: &[u8], authcid: &[u8], passwd: &[u8]) -> Box<dyn SaslLogic> {
        match self {
            PasswordMechanism::Plain => Box::new(PlainLogic::new(authzid, authcid, passwd)),
        }
    }
}

/// Configuration for general username+password authentication.
///
/// Dispatcher for commonly-supported forms of password authentication.
/// In a VERY COMMON worst case, transmits the password in the clear.
/// Additionally, other mechanisms may still permit practical attacks
/// against a user's password. Prefer to use this over secure connections
/// (and ideally encourage end users to use client certificate auth instead).
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde_derive::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(bound(deserialize = "'de: 'static, S: LoadSecret + serde::de::Deserialize<'de>"))
)]
pub struct Password<S> {
    /// The set of authentication methods to FORBID.
    ///
    /// It's usually not necessary to set this as most IRC connections are likely to be
    /// either local or over a TLS connection.
    #[cfg_attr(feature = "serde", serde(default))]
    pub deny_methods: BTreeSet<PasswordMechanism>,
    /// Who to log in as, or empty to log in as the user specified in `authcid`.
    #[cfg_attr(feature = "serde", serde(default))]
    pub authzid: NoNul<'static>,
    /// Whose credentials to use for logging in.
    pub authcid: NoNul<'static>,
    /// The password.
    pub passwd: Secret<NoNul<'static>, S>,
}

impl<S> Password<S> {
    /// Creates `self` from a username and password combination.
    ///
    /// This type has fields that are typically not required for normal use.
    /// This function initializes those fields accordingly.
    pub const fn new(username: NoNul<'static>, passwd: Secret<NoNul<'static>, S>) -> Self {
        Password {
            deny_methods: BTreeSet::new(),
            authzid: NoNul::empty(),
            authcid: username,
            passwd,
        }
    }
}

impl<S> Sasl for Password<S> {
    fn logic(&self) -> Vec<Box<dyn SaslLogic>> {
        PasswordMechanism::full_set()
            .difference(&self.deny_methods)
            .copied()
            .map(|mech| mech.logic(&self.authzid, &self.authcid, &self.passwd))
            .collect()
    }
}

/// Configuration for PLAIN authentication.
///
/// It is generally recommended to use [`Password`] instead.
///
/// Transmits the password in the clear;
/// do not use this without some form of secure transport, like TLS.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde_derive::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(bound(deserialize = "'de: 'static, S: LoadSecret + serde::de::Deserialize<'de>"))
)]
pub struct Plain<S> {
    /// Who to log in as, or empty to log in as the user specified in `authcid`.
    #[cfg_attr(feature = "serde", serde(default))]
    pub authzid: NoNul<'static>,
    /// Whose credentials to use for logging in.
    pub authcid: NoNul<'static>,
    /// The password.
    pub passwd: Secret<NoNul<'static>, S>,
}

impl<S> Plain<S> {
    /// Creates `self` from a username and password combination.
    ///
    /// This type has fields that are typically not required for normal use.
    /// This function initializes those fields accordingly.
    pub const fn new(username: NoNul<'static>, passwd: Secret<NoNul<'static>, S>) -> Self {
        Plain { authzid: NoNul::empty(), authcid: username, passwd }
    }
}

impl<S: LoadSecret + 'static> Sasl for Plain<S> {
    fn logic(&self) -> Vec<Box<dyn SaslLogic>> {
        let authzid = self.authzid.as_bytes();
        let authcid = self.authcid.as_bytes();
        let passwd = self.passwd.as_bytes();
        vec![Box::new(PlainLogic::new(authzid, authcid, passwd))]
    }
}

struct PlainLogic(SecretBuf);

impl PlainLogic {
    pub fn new(authzid: &[u8], authcid: &[u8], passwd: &[u8]) -> Self {
        // Add 2 for separating nul bytes.
        let mut data = SecretBuf::with_capacity(2 + passwd.len() + authcid.len() + authzid.len());
        data.push_cstr(authzid);
        data.push_cstr(authcid);
        data.push_slice(passwd);
        PlainLogic(data)
    }
}

impl SaslLogic for PlainLogic {
    fn name(&self) -> Arg<'static> {
        SASL_PLAIN_NAME.clone()
    }

    fn reply<'a>(
        &'a mut self,
        data: &[u8],
        output: &mut SecretBuf,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        if !data.is_empty() {
            return Err("non-empty server message".into());
        }
        if self.0.is_empty() {
            return Err("already sent auth".into());
        }
        std::mem::swap(output, &mut self.0);
        Ok(())
    }
}
