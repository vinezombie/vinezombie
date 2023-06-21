use super::{BoxedErr, Sasl, SaslLogic};
use crate::{
    ircmsg::ClientMsg,
    known::cmd::AUTHENTICATE,
    string::{Arg, Line},
};
use std::collections::BTreeSet;

/// Handler for SASL authentication.
pub struct Handler {
    logic: Box<dyn SaslLogic>,
    decoder: crate::string::base64::ChunkDecoder,
}

/// All the possible errors that can occur during SASL authentication.
#[derive(Debug)]
pub enum HandlerError {
    /// The client requested a mechanism that isn't supported.
    /// The server supports the inclded mechanisms.
    WrongMechanism(BTreeSet<Arg<'static>>),
    /// Authentication failed.
    Fail(Line<'static>),
    /// The server's implementation of a SASL mechanism is broken.
    Broken(BoxedErr),
    /// An I/O error occurred.
    Io(std::io::Error),
}

impl From<HandlerError> for std::io::Error {
    fn from(value: HandlerError) -> Self {
        use std::io::{Error, ErrorKind};
        match value {
            HandlerError::WrongMechanism(_) => {
                Error::new(ErrorKind::Unsupported, value.to_string())
            }
            HandlerError::Fail(_) => Error::new(ErrorKind::PermissionDenied, value.to_string()),
            HandlerError::Broken(e) => Error::new(ErrorKind::InvalidData, e),
            HandlerError::Io(e) => e,
        }
    }
}

impl std::fmt::Display for HandlerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HandlerError::WrongMechanism(_) => write!(f, "unsupported mechanism"),
            HandlerError::Fail(reason) => reason.fmt(f),
            HandlerError::Broken(_) => write!(f, "broken mechanism"),
            HandlerError::Io(_) => write!(f, "io failure"),
        }
    }
}

impl std::error::Error for HandlerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            HandlerError::Broken(brk) => Some(&**brk),
            HandlerError::Io(io) => Some(io),
            _ => None,
        }
    }
}

impl Handler {
    /// Creates a new authenticator.
    pub fn new(sasl: &(impl Sasl + ?Sized)) -> Result<(ClientMsg<'static>, Self), BoxedErr> {
        let auth = Handler {
            logic: sasl.logic()?,
            decoder: crate::string::base64::ChunkDecoder::new(400),
        };
        Ok((super::msg_auth(sasl), auth))
    }
    /// Handles a server message sent during SASL authentication.
    /// Returns `Ok(true)` if authenticated, or `Ok(false)` if more messages are required.
    pub fn handle(
        &mut self,
        msg: &crate::ircmsg::ServerMsg<'_>,
        mut send_fn: impl FnMut(ClientMsg<'static>) -> Result<(), std::io::Error>,
    ) -> Result<bool, HandlerError> {
        use crate::string::base64::ChunkEncoder;
        match msg.kind.as_str() {
            "AUTHENTICATE" => {
                let res = if let Some(first) = msg.args.args().first() {
                    self.decoder.add(first.as_bytes())
                } else {
                    Some(self.decoder.decode())
                };
                if let Some(res) = res {
                    let chal = res.map_err(|e| HandlerError::Broken(e.into()))?;
                    let reply = self.logic.reply(&chal).map_err(HandlerError::Broken)?;
                    for chunk in ChunkEncoder::new(reply, 400, true) {
                        let mut msg = ClientMsg::new_cmd(AUTHENTICATE);
                        msg.args.add(chunk);
                        send_fn(msg).map_err(HandlerError::Io)?;
                    }
                }
                Ok(false)
            }
            // Ignore 901.
            "900" | "903" | "907" => Ok(true),
            "902" | "904" | "905" | "906" => {
                let reason = msg.args.split_last().1.cloned().unwrap_or_default();
                Err(HandlerError::Fail(reason.owning()))
            }
            "908" => {
                #[allow(clippy::mutable_key_type)]
                let set = msg.args.args().iter().map(|a| a.clone().owning()).collect();
                Err(HandlerError::WrongMechanism(set))
            }
            _ => Ok(false),
        }
    }
}
