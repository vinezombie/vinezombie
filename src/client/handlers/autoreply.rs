use crate::{
    client::{
        channel::{ChannelSpec, ClosedSender, Sender},
        queue::QueueEditGuard,
        ClientState, Handler, SelfMadeHandler,
    },
    ircmsg::{ClientMsg, MaybeCtcp, ServerMsg},
    names::cmd::{NOTICE, PRIVMSG},
    string::{Line, Word},
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
        _: &mut ClientState,
        mut queue: QueueEditGuard<'_>,
        _: crate::client::channel::SenderRef<'_, Self::Value>,
    ) -> std::ops::ControlFlow<()> {
        // TODO: Should probably consider length limits.
        let Ok(msg) = msg.parse_as(PRIVMSG) else {
            return std::ops::ControlFlow::Continue(());
        };
        let msg = msg.map(MaybeCtcp::from);
        let Some(source) = msg.source else {
            // Wat?
            return std::ops::ControlFlow::Continue(());
        };
        match msg.value.cmd.as_bytes() {
            b"VERSION" if !self.version.is_empty() => {
                let mut msg = ClientMsg::new(NOTICE);
                let mut args = msg.args.edit();
                args.add_word(source.nick.clone().owning());
                args.add(MaybeCtcp { cmd: Word::from_str("VERSION"), body: self.version.clone() });
                queue.push(msg);
            }
            b"SOURCE" if !self.source.is_empty() => {
                let mut msg = ClientMsg::new(NOTICE);
                let mut args = msg.args.edit();
                args.add_word(source.nick.clone().owning());
                args.add(MaybeCtcp { cmd: Word::from_str("SOURCE"), body: self.source.clone() });
                queue.push(msg);
            }
            _ => (),
        }
        std::ops::ControlFlow::Continue(())
    }
}

impl SelfMadeHandler for CtcpVersion {
    type Receiver<Spec: ChannelSpec> = ();

    fn queue_msgs(&self, _: &ClientState, _: QueueEditGuard<'_>) {}

    fn make_channel<Spec: ChannelSpec>(
        _: &Spec,
    ) -> (Box<dyn Sender<Value = Self::Value> + Send>, Self::Receiver<Spec>) {
        (Box::<ClosedSender<_>>::default(), ())
    }
}
