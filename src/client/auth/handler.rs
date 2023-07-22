use super::{Sasl, SaslLogic};
use crate::{
    client::{ClientMsgSink, HandlerOk, HandlerResult},
    consts::cmd::AUTHENTICATE,
    ircmsg::ClientMsg,
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
    Broken(Box<dyn std::error::Error + Send + Sync>),
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
            HandlerError::Fail(e) => Error::new(ErrorKind::PermissionDenied, e.to_utf8_lossy()),
            HandlerError::Broken(e) => Error::new(ErrorKind::InvalidData, e),
            HandlerError::Io(e) => e,
        }
    }
}
impl From<std::io::Error> for HandlerError {
    fn from(value: std::io::Error) -> Self {
        HandlerError::Io(value)
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
    /// Creates a new authenticator from a [`SaslLogic`] implementation.
    pub fn from_logic(logic: Box<dyn SaslLogic>) -> Self {
        Handler { logic, decoder: crate::string::base64::ChunkDecoder::new(400) }
    }
    /// Creates a new authenticator from a [`Sasl`] implementation.
    ///
    /// For convenience, also returns the message to send to initiate authentication.
    pub fn from_sasl(sasl: &(impl Sasl + ?Sized)) -> std::io::Result<(ClientMsg<'static>, Self)> {
        let auth = Handler {
            logic: sasl.logic()?,
            decoder: crate::string::base64::ChunkDecoder::new(400),
        };
        let mut msg = ClientMsg::new_cmd(crate::consts::cmd::AUTHENTICATE);
        msg.args.edit().add_word(sasl.name());
        Ok((msg, auth))
    }
    /// Handles a server message sent during SASL authentication.
    ///
    /// Upon returning an `Ok(HandlerOk::Value(_))`, authentication has completed successfully.
    pub fn handle(
        &mut self,
        msg: &crate::ircmsg::ServerMsg<'_>,
        mut sink: impl ClientMsgSink<'static>,
    ) -> HandlerResult<(), std::convert::Infallible, HandlerError> {
        use crate::string::base64::ChunkEncoder;
        match msg.kind.as_str() {
            "AUTHENTICATE" => {
                let res = if let Some(first) = msg.args.words().first() {
                    self.decoder.add(first.as_bytes())
                } else {
                    Some(self.decoder.decode())
                };
                if let Some(res) = res {
                    let chal = res.map_err(|e| HandlerError::Broken(e.into()))?;
                    let reply = self.logic.reply(&chal).map_err(HandlerError::Broken)?;
                    for chunk in ChunkEncoder::new(reply, 400, true) {
                        let mut msg = ClientMsg::new_cmd(AUTHENTICATE);
                        msg.args.edit().add_word(chunk);
                        sink.send(msg).map_err(HandlerError::Io)?;
                    }
                }
                Ok(HandlerOk::NeedMore)
            }
            // Ignore 901.
            "900" | "903" | "907" => Ok(HandlerOk::Value(())),
            "902" | "904" | "905" | "906" => {
                let reason = msg.args.split_last().1.cloned().unwrap_or_default();
                Err(HandlerError::Fail(reason.owning()))
            }
            "908" => {
                #[allow(clippy::mutable_key_type)]
                let set = msg.args.split_last().0.iter().map(|a| a.clone().owning()).collect();
                Err(HandlerError::WrongMechanism(set))
            }
            _ => Ok(HandlerOk::Ignored),
        }
    }
}

impl crate::client::Handler for Handler {
    type Value = ();
    type Warning = std::convert::Infallible;
    type Error = HandlerError;

    fn handle(
        &mut self,
        msg: &crate::ircmsg::ServerMsg<'static>,
        queue: &mut crate::client::Queue<'static>,
    ) -> HandlerResult<Self::Value, Self::Warning, Self::Error> {
        self.handle(msg, queue)
    }
}
