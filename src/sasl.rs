//! SASL mechanisms and related types.

use std::{error::Error, ffi::CString, sync::Arc};

/// Types that can store sensitive byte strings.
pub trait Secret {
    /// Appends the secret's bytes to the provided string.
    ///
    /// This method may fail if retrieving the secret fails.
    fn append_to(&self, data: &mut Vec<u8>) -> Result<(), Box<dyn Error>>;
    /// Returns a hint for how many bytes should be preallocated for this secret.
    fn len_hint(&self) -> usize {
        0
    }
}

/// Zero-added-security implementation of `Secret`.
impl Secret for String {
    fn append_to(&self, data: &mut Vec<u8>) -> Result<(), Box<dyn Error>> {
        data.extend(self.as_bytes());
        Ok(())
    }
    fn len_hint(&self) -> usize {
        self.len()
    }
}

/// The type used for [SaslLogic] replies.
///
/// Bundles the body of the reply with a continuation for the logic.
type SaslReply = (Vec<u8>, Box<dyn SaslLogic>);

/// The logic of a SASL mechanism.
///
/// This trait uses a similar pattern to [crate::handler::Handler],
/// where every state change is explicitly modeled but
/// the state may change types during execution.
pub trait SaslLogic {
    /// Handles data sent by the server.
    fn reply(self: Box<Self>, data: Vec<u8>) -> Result<SaslReply, Box<dyn Error>>;
}

/// [SaslLogic] that marks the end of expected server messages.
pub struct SaslLogicDone;

impl SaslLogic for SaslLogicDone {
    fn reply(self: Box<Self>, _: Vec<u8>) -> Result<SaslReply, Box<dyn Error>> {
        Err("unexpected reply from server".into())
    }
}

/// SASL mechanisms.
pub trait Sasl: std::fmt::Debug {
    /// The name of this mechanism, as the client requests it.
    fn name(&self) -> &'static str;
    /// Returns the logic for this mechanism as a [SaslLogic].
    fn logic(&self) -> Box<dyn SaslLogic>;
}

/// The [EXTERNAL SASL mechanism](https://www.rfc-editor.org/rfc/rfc4422#appendix-A).
///
/// This is what is used when authentication occurs out-of-band,
/// such as when using TLS client certificate authentication.
#[derive(Clone, Copy, Default, Debug)]
pub struct External;

struct ExternalLogic;

impl SaslLogic for ExternalLogic {
    fn reply(self: Box<Self>, _: Vec<u8>) -> Result<SaslReply, Box<dyn Error>> {
        // TODO: Strictness?
        Ok((Vec::new(), Box::new(SaslLogicDone)))
    }
}

impl Sasl for External {
    fn name(&self) -> &'static str {
        "EXTERNAL"
    }
    fn logic(&self) -> Box<dyn SaslLogic> {
        Box::new(ExternalLogic)
    }
}

/// The [PLAIN SASL mechanism](https://www.rfc-editor.org/rfc/rfc4616).
///
/// This is what is used for typical username/password authentication.
#[derive(Clone)]
pub struct Plain<S> {
    authzid: Arc<CString>,
    authcid: Arc<CString>,
    passwd: Arc<S>,
}

impl<S: Secret> Plain<S> {
    /// Creates a [Plain] that logs in to the specified account.
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
    /// Creates a [Plain] that logs into the account specified by authzid
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

struct PlainLogic<S> {
    authzid: Arc<CString>,
    authcid: Arc<CString>,
    passwd: Arc<S>,
}

impl<S: Secret> SaslLogic for PlainLogic<S> {
    fn reply(self: Box<Self>, mut data: Vec<u8>) -> Result<SaslReply, Box<dyn Error>> {
        // TODO: Strictness?
        data.clear();
        let authzid = self.authzid.as_bytes_with_nul();
        let authcid = self.authcid.as_bytes_with_nul();
        data.reserve(self.passwd.len_hint() + authcid.len() + authzid.len());
        data.extend(authzid);
        data.extend(authcid);
        self.passwd.append_to(&mut data)?;
        Ok((data, Box::new(SaslLogicDone)))
    }
}

impl<S: Secret + 'static> Sasl for Plain<S> {
    fn name(&self) -> &'static str {
        "PLAIN"
    }
    fn logic(&self) -> Box<dyn SaslLogic> {
        Box::new(PlainLogic {
            authzid: self.authzid.clone(),
            authcid: self.authcid.clone(),
            passwd: self.passwd.clone(),
        })
    }
}
