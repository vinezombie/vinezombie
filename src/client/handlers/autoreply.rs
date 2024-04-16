use crate::{
    client::{channel::ClosedSender, queue::QueueEditGuard, Handler, SelfMadeHandler},
    ircmsg::{ClientMsg, MaybeCtcp, ServerMsg},
    names::cmd::{NOTICE, PRIVMSG},
    string::Line,
};

/// Handler for static replies to CTCP VERSION and SOURCE.
#[derive(Clone, Debug)]
pub struct CtcpVersion {
    /// The response to the `VERSION` query, if non-empty.
    pub version: Line<'static>,
    /// The response to the `SOURCE` query, if non-empty.
    pub source: Line<'static>,
}

/// Creates a [`CtcpVersion`] handler using the current package's information.
#[macro_export]
macro_rules! ctcp_version_handler {
    () => {{
        use ::vinezombie::string::Line;
        let version =
            Line::from_bytes(concat!(env!("CARGO_PKG_NAME"), " v", env!("CARGO_PKG_VERSION")))
                .unwrap_or_default();
        let source = Line::from_bytes(env!("CARGO_PKG_REPOSITORY")).unwrap_or_default();
        ::vinezombie::client::handlers::CtcpVersion { version, source }
    }};
}

impl Handler for CtcpVersion {
    type Value = ();

    fn handle(
        &mut self,
        msg: &ServerMsg<'_>,
        mut queue: QueueEditGuard<'_>,
        _: crate::client::channel::SenderRef<'_, Self::Value>,
    ) -> bool {
        let Ok(msg) = msg.parse_as(PRIVMSG) else {
            return false;
        };
        let msg = msg.map(MaybeCtcp::from);
        let Some(source) = msg.source else {
            // Wat?
            return false;
        };
        match msg.value.0.as_bytes() {
            b"VERSION" if !self.version.is_empty() => {
                let mut msg = ClientMsg::new(NOTICE);
                let mut args = msg.args.edit();
                args.add_word(source.nick.clone().owning());
                args.add(self.version.clone());
                queue.push(msg);
            }
            b"SOURCE" if !self.source.is_empty() => {
                let mut msg = ClientMsg::new(NOTICE);
                let mut args = msg.args.edit();
                args.add_word(source.nick.clone().owning());
                args.add(self.source.clone());
                queue.push(msg);
            }
            _ => (),
        }
        false
    }
}

impl SelfMadeHandler for CtcpVersion {
    type Receiver<Spec: crate::client::channel::ChannelSpec> = ();

    fn queue_msgs(&self, _: QueueEditGuard<'_>) {}

    fn make_channel<Spec: crate::client::channel::ChannelSpec>(
        _: &Spec,
    ) -> (Box<dyn crate::client::channel::Sender<Value = Self::Value> + Send>, Self::Receiver<Spec>)
    {
        (Box::new(ClosedSender::default()), ())
    }
}
