use std::ops::ControlFlow;

use crate::{
    client::{
        channel::{ChannelSpec, ClosedSender, Sender},
        queue::QueueEditGuard,
        state::ClientSource,
        ClientState, Handler, SelfMadeHandler,
    },
    error::ParseError,
    ircmsg::{ClientMsg, Source, UserHost},
    names::cmd::USERHOST,
    string::{Arg, Nick, User, Word},
};

/// Handler for automatically updating this client's [`ClientSource`].
///
/// It is strongly recommended to use this handler
/// and to add it before any other handlers that send messages.
///
/// If the client's nick is known,
/// this handler begins by sending a [`USERHOST`] message to query the client's [`UserHost`].
/// Otherwise, it remains in the background and updates state.
#[derive(Default)]
pub struct TrackClientSource {}

impl TrackClientSource {
    /// Creates a new `TrackSelf` handler.
    pub fn new() -> Self {
        TrackClientSource {}
    }
}

fn get_client_source(state: &mut ClientState) -> ControlFlow<(), &mut Source<'static>> {
    match state.get_mut::<ClientSource>() {
        Some(v) => ControlFlow::Continue(v),
        None => ControlFlow::Break(()),
    }
}

// TODO: Would be nice to extract RPL_USERHOST parsing.
fn parse_userhost_item<'a>(
    item: Arg<'a>,
    nick: &Nick<'_>,
) -> Result<UserHost<'a>, Option<ParseError>> {
    let mut splitter = crate::string::Splitter::new(item);
    match splitter.save_end().until_byte_eq(b'=').until_byte_eq(b'*').string::<Nick>(true) {
        Ok(n) if n == *nick => (),
        _ => return Err(None),
    }
    let _is_op = match splitter.next_byte() {
        Some(b'=') => false,
        Some(b'*') => match splitter.next_byte() {
            Some(b'=') => true,
            other => {
                return Err(Some(ParseError::InvalidField(
                    "is_op".into(),
                    format!("expected =, got {other:?}").into(),
                )))
            }
        },
        other => {
            return Err(Some(ParseError::InvalidField(
                "is_op".into(),
                format!("expected * or =, got {other:?}").into(),
            )))
        }
    };
    let _is_away = match splitter.next_byte() {
        Some(b'+') => false,
        Some(b'-') => true,
        other => {
            return Err(Some(ParseError::InvalidField(
                "is_away".into(),
                format!("expected + or -, got {other:?}").into(),
            )))
        }
    };
    UserHost::parse(splitter.rest_or_default::<Word>()).map_err(Some)
}

impl Handler for TrackClientSource {
    type Value = ();

    fn handle(
        &mut self,
        msg: &crate::ircmsg::ServerMsg<'_>,
        state: &mut ClientState,
        _: QueueEditGuard<'_>,
        _: crate::client::channel::SenderRef<'_, Self::Value>,
    ) -> ControlFlow<()> {
        match msg.kind.as_str() {
            // RPL_USERHOST
            "302" => {
                if let Some([_, items @ ..]) = msg.args.all() {
                    let src = get_client_source(state)?;
                    for item in items {
                        let userhost = match parse_userhost_item(item.clone(), &src.nick) {
                            Ok(uh) => uh,
                            // TODO: Log warning on ParseError?
                            Err(_) => continue,
                        };
                        src.userhost = Some(userhost.owning());
                        state.update_source_len();
                        break;
                    }
                }
            }
            "NICK" => {
                if let Some([nick]) = msg.args.all() {
                    let src = get_client_source(state)?;
                    let Ok(nick) = Nick::from_super(nick.clone()) else {
                        // TODO: Log warning?
                        return ControlFlow::Continue(());
                    };
                    match msg.source.as_ref() {
                        Some(m_src) if m_src.nick == src.nick => {
                            src.nick = nick.owning();
                            state.update_source_len();
                        }
                        _ => (),
                    }
                }
            }
            "CHGHOST" => {
                if let Some([user, host]) = msg.args.all() {
                    let src = get_client_source(state)?;
                    match msg.source.as_ref() {
                        Some(m_src) if m_src.nick == src.nick => {
                            let user = match User::from_super(user.clone()) {
                                Ok(u) => u.owning(),
                                // TODO: Log warning?
                                Err(_) => return ControlFlow::Continue(()),
                            };
                            src.userhost = Some(UserHost {
                                user: Some(user),
                                host: host.clone().owning().into(),
                            });
                            state.update_source_len();
                        }
                        _ => (),
                    }
                }
            }
            _ => (),
        }
        ControlFlow::Continue(())
    }
}

impl SelfMadeHandler for TrackClientSource {
    type Receiver<Spec: ChannelSpec> = ();

    fn queue_msgs(&self, state: &ClientState, mut queue: QueueEditGuard<'_>) {
        if let Some(src) = state.get::<ClientSource>() {
            let mut msg = ClientMsg::new(USERHOST);
            msg.args.edit().add(src.nick.clone());
            queue.push(msg);
        }
    }

    fn make_channel<Spec: ChannelSpec>(
        _spec: &Spec,
    ) -> (Box<dyn Sender<Value = Self::Value> + Send>, Self::Receiver<Spec>) {
        (Box::<ClosedSender<_>>::default(), ())
    }
}
