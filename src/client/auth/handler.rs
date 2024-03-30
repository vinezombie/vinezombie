use super::{Sasl, SaslLogic};
use crate::{
    client::ClientMsgSink,
    ircmsg::ClientMsg,
    names::cmd::AUTHENTICATE,
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
        }
    }
}

impl std::fmt::Display for HandlerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HandlerError::WrongMechanism(_) => write!(f, "unsupported mechanism"),
            HandlerError::Fail(reason) => reason.fmt(f),
            HandlerError::Broken(_) => write!(f, "broken mechanism"),
        }
    }
}

impl std::error::Error for HandlerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            HandlerError::Broken(brk) => Some(&**brk),
            _ => None,
        }
    }
}

/// [`MakeHandler`][crate::client::MakeHandler] for any [`Sasl`] implementation.
#[derive(Clone, Copy, Debug, Default)]
pub struct Authenticate;

impl<'a, T: Sasl> crate::client::MakeHandler<&'a T> for Authenticate {
    type Value = Result<(), HandlerError>;

    type Error = std::io::Error;

    type Receiver<Spec: crate::client::channel::ChannelSpec> = Spec::Oneshot<Self::Value>;

    fn make_handler(
        &self,
        mut queue: crate::client::QueueEditGuard<'_>,
        sasl: &'a T,
    ) -> Result<impl crate::client::Handler<Value = Self::Value>, Self::Error> {
        let (msg, handler) = Handler::from_sasl(sasl)?;
        queue.push(msg);
        Ok(handler)
    }

    fn make_channel<Spec: crate::client::channel::ChannelSpec>(
        spec: &Spec,
    ) -> (
        std::sync::Arc<dyn crate::client::channel::Sender<Value = Self::Value>>,
        Self::Receiver<Spec>,
    ) {
        spec.new_oneshot()
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
        let mut msg = ClientMsg::new(crate::names::cmd::AUTHENTICATE);
        msg.args.edit().add_word(sasl.name());
        Ok((msg, auth))
    }
    /// Handles a server message sent during SASL authentication.
    ///
    /// Upon returning an `Ok(true)`, authentication has completed successfully.
    /// A return value of `Ok(false)` means more messages are required.
    pub fn handle(
        &mut self,
        msg: &crate::ircmsg::ServerMsg<'_>,
        mut sink: impl ClientMsgSink<'static>,
    ) -> Result<bool, HandlerError> {
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
                        let mut msg = ClientMsg::new(AUTHENTICATE);
                        msg.args.edit().add_word(chunk);
                        sink.send(msg);
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
                let set = msg.args.split_last().0.iter().map(|a| a.clone().owning()).collect();
                Err(HandlerError::WrongMechanism(set))
            }
            _ => Ok(false),
        }
    }
}

impl crate::client::Handler for Handler {
    type Value = Result<(), HandlerError>;

    fn handle(
        &mut self,
        msg: &crate::ircmsg::ServerMsg<'_>,
        mut queue: crate::client::QueueEditGuard<'_>,
        mut channel: crate::client::channel::SenderRef<'_, Self::Value>,
    ) -> bool {
        match self.handle(msg, &mut queue) {
            Ok(false) => false,
            v => {
                channel.send(v.and(Ok(())));
                true
            }
        }
    }
}
