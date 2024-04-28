use super::{Sasl, SaslLogic, SaslQueue};
use crate::{
    client::{auth::msg_abort, ClientMsgSink, NoHandler},
    ircmsg::ClientMsg,
    names::cmd::AUTHENTICATE,
    string::{Arg, Line, SecretBuf},
};

/// Handler for SASL authentication.
pub struct Handler {
    queue: SaslQueue,
    logic: Box<dyn SaslLogic>,
    decoder: crate::string::base64::ChunkDecoder,
}

/// All the possible errors that can occur during SASL authentication.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum HandlerError {
    /// The last available authenticator was ruled out by a broken server implementation.
    Broken(Arg<'static>),
    /// The last available authenticator was ruled out by the server not supporting it.
    Unsupported,
    /// The last available authenticator failed, or the account is frozen.
    Fail(Line<'static>),
}

impl From<HandlerError> for std::io::Error {
    fn from(value: HandlerError) -> Self {
        use std::io::{Error, ErrorKind};
        match value {
            HandlerError::Fail(e) => Error::new(ErrorKind::PermissionDenied, e.to_utf8_lossy()),
            HandlerError::Broken(_) => Error::new(ErrorKind::InvalidData, value.to_string()),
            HandlerError::Unsupported => Error::new(ErrorKind::Unsupported, value.to_string()),
        }
    }
}

impl std::fmt::Display for HandlerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HandlerError::Fail(reason) => write!(f, "login failed: {reason}"),
            HandlerError::Unsupported => write!(f, "no supported mechanisms"),
            HandlerError::Broken(m) => write!(f, "server has broken {m} implementation"),
        }
    }
}

impl std::error::Error for HandlerError {}

impl crate::client::MakeHandler<SaslQueue> for crate::names::cmd::AUTHENTICATE {
    type Value = Result<(), HandlerError>;

    type Error = crate::client::handler::NoHandler;

    type Receiver<Spec: crate::client::channel::ChannelSpec> = Spec::Oneshot<Self::Value>;

    type Handler = Handler;

    fn make_handler(
        self,
        _: &crate::client::ClientState,
        mut queue: crate::client::queue::QueueEditGuard<'_>,
        mut sasl_queue: SaslQueue,
    ) -> Result<Handler, Self::Error> {
        let sasl = sasl_queue.pop().ok_or(NoHandler)?;
        let retval = Handler::new(sasl, sasl_queue);
        queue.push(retval.auth_msg());
        Ok(retval)
    }

    fn make_channel<Spec: crate::client::channel::ChannelSpec>(
        spec: &Spec,
    ) -> (Box<dyn crate::client::channel::Sender<Value = Self::Value> + Send>, Self::Receiver<Spec>)
    {
        spec.new_oneshot()
    }
}

impl<'a, T: Sasl> crate::client::MakeHandler<&'a T> for crate::names::cmd::AUTHENTICATE {
    type Value = Result<(), HandlerError>;

    type Error = crate::client::handler::NoHandler;

    type Receiver<Spec: crate::client::channel::ChannelSpec> = Spec::Oneshot<Self::Value>;

    type Handler = Handler;

    fn make_handler(
        self,
        _: &crate::client::ClientState,
        mut queue: crate::client::queue::QueueEditGuard<'_>,
        sasl: &'a T,
    ) -> Result<Handler, Self::Error> {
        let mut sasl_queue: SaslQueue = sasl.logic().into();
        let sasl = sasl_queue.pop().ok_or(NoHandler)?;
        let retval = Handler::new(sasl, sasl_queue);
        queue.push(retval.auth_msg());
        Ok(retval)
    }

    fn make_channel<Spec: crate::client::channel::ChannelSpec>(
        spec: &Spec,
    ) -> (Box<dyn crate::client::channel::Sender<Value = Self::Value> + Send>, Self::Receiver<Spec>)
    {
        spec.new_oneshot()
    }
}

impl Handler {
    /// Creates a new authenticator from a [`SaslLogic`] implementation and a
    /// (potentially empty) queue.
    /// Additionally returns the message to send to initiate authentication.
    pub fn new(logic: Box<dyn SaslLogic>, queue: SaslQueue) -> Self {
        Handler { queue, logic, decoder: crate::string::base64::ChunkDecoder::new(400) }
    }
    /// Attempts to create a new authenticator directly from a [`SaslQueue`].
    /// Returns `None` if the queue is empty.
    pub fn from_queue(mut queue: SaslQueue) -> Option<Self> {
        let logic = queue.pop()?;
        Some(Self::new(logic, queue))
    }
    /// Creates an auth message for the current [`SaslLogic`].
    ///
    /// If you are manually driving this handler, this should typically
    /// only need to be called once: at the start.
    pub fn auth_msg(&self) -> ClientMsg<'static> {
        let mut msg = ClientMsg::new(crate::names::cmd::AUTHENTICATE);
        msg.args.edit().add_word(self.logic.name().clone());
        msg
    }

    /// Constrains SASL authenticators to only support mechanisms whose names return `true`
    /// when passed to the provided function.
    ///
    /// Returns:
    /// - `Some(true)` if the current authenticator was removed by this function,
    /// meaning authentication needs to be restarted.
    /// - `Some(false)` if all authenticators were removed by this function.
    /// - `None` if authentication can continue normally.
    pub fn retain(&mut self, supported: &(impl Fn(&Arg<'_>) -> bool + ?Sized)) -> Option<bool> {
        self.queue.retain(supported);
        if supported(&self.logic.name()) {
            None
        } else if let Some(new_logic) = self.queue.pop() {
            self.logic = new_logic;
            Some(true)
        } else {
            Some(false)
        }
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
                    let chal = res.map_err(|_e| {
                        #[cfg(feature = "tracing")]
                        tracing::error!("base64 decode error: {_e}");
                        sink.send(msg_abort());
                        HandlerError::Broken(Arg::from_str("base64"))
                    })?;
                    let mut buf = SecretBuf::with_capacity(self.logic.size_hint());
                    if let Err(_e) = self.logic.reply(&chal, &mut buf) {
                        #[cfg(feature = "tracing")]
                        tracing::error!("server's SASL {} is broken: {_e}", self.logic.name());
                        sink.send(msg_abort());
                        // Now to rule out this mechanism from all further auth attempts.
                        let name = self.logic.name();
                        self.queue.retain(&|ln| name != *ln);
                        return if let Some(new_logic) = self.queue.pop() {
                            self.logic = new_logic;
                            // We can continue, but we need to wait for the server to
                            // acknowledge that we're stopping before sending AUTHENTICATE.
                            Ok(false)
                        } else {
                            Err(HandlerError::Broken(name))
                        };
                    }
                    for chunk in ChunkEncoder::new(buf, 400, true) {
                        let mut msg = ClientMsg::new(AUTHENTICATE);
                        msg.args.edit().add_word(chunk);
                        sink.send(msg);
                    }
                }
                Ok(false)
            }
            // Auth failed or the account was restricted.
            "902" | "904" => {
                // In a more account-aware system, could purge all authenticators that are
                // meant to log in to the same account on a 902.
                if let Some(next_logic) = self.queue.pop() {
                    self.logic = next_logic;
                    sink.send(self.auth_msg());
                    Ok(false)
                } else {
                    let reason = msg.args.split_last().1.cloned().unwrap_or_default().owning();
                    Err(HandlerError::Fail(reason))
                }
            }
            // Somehow we sent more than 400 bytes in an AUTHENTICATE message?
            "905" => {
                // Heresy, it's the server that's wrong!
                Err(HandlerError::Broken(Arg::from_str("counting")))
            }
            // We asked for authentication to stop.
            "906" => {
                // Since we're here, we're trying again.
                // The authenticator was already cycled earlier, so we just send the message.
                sink.send(self.auth_msg());
                Ok(false)
            }
            // Server is telling us something about the supported mechanisms.
            "908" => {
                // Let's assume that these apply to all accounts we might try to log in to.
                #[allow(clippy::mutable_key_type)]
                let set: std::collections::BTreeSet<_> =
                    msg.args.split_last().0.iter().map(|a| a.clone().owning()).collect();
                self.queue.retain(&|ln| set.contains(ln));
                // If the mechanism is NOT supported,
                // the server needs to error with a 904, which causes this handler to
                // cycle to the next authenticator.
                // However, we can short-circuit this if the queue is empty AND
                // the authenticator is unsupported.
                if self.queue.is_empty() && !set.contains(&self.logic.name()) {
                    Err(HandlerError::Unsupported)
                } else {
                    Ok(false)
                }
            }
            // Various ways of telling us "we're logged in".
            // Something else should properly parse the 900.
            "900" | "903" | "907" => Ok(true),
            // Ignore 901, the "logged out" message.
            _ => Ok(false),
        }
    }
}

impl crate::client::Handler for Handler {
    type Value = Result<(), HandlerError>;

    fn handle(
        &mut self,
        msg: &crate::ircmsg::ServerMsg<'_>,
        _: &mut crate::client::ClientState,
        mut queue: crate::client::queue::QueueEditGuard<'_>,
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
