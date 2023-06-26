/// The return type of the `handle` methods on types in this module.
pub type HandlerResult<T, W, E> = Result<HandlerOk<T, W>, E>;

/// All the possible forms of success for a single handler step.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Default)]
pub enum HandlerOk<T, W> {
    /// The provided message isn't relevant to this handler and has been ignored.
    #[default]
    Ignored,
    /// More messages are required.
    NeedMore,
    /// More messages are required.
    /// Additionally, a handler step has errored but recovered successfully.
    Warning(W),
    /// The handler has yielded a value.
    Value(T),
}

// Lifetime screwiness with handler methods on the ClientMsgSink param
// means that using a macro_rules.

/// Runs a handler function to completion.
#[macro_export]
macro_rules! run_handler {
    ($conn:ident, $queue:ident, $handler:ident, $handler_fn:expr) => {{
        use vinezombie::client::{conn::Connection, HandlerOk, HandlerResult};
        use vinezombie::ircmsg::ServerMsg;
        let mut buf = Vec::new();
        loop {
            let msg = $queue.pop(|dur| $conn.set_read_timeout(dur).unwrap());
            if let Some(msg) = msg {
                msg.send_to($conn.as_write(), &mut buf)?;
                continue;
            }
            match ServerMsg::read_owning_from($conn.as_bufread(), &mut buf) {
                Ok(msg) => {
                    let result = $handler_fn(&mut $handler, &msg, &mut $queue);
                    if let HandlerOk::Value(val) = result? {
                        break Ok(val);
                    }
                }
                Err(e) => break Err(e),
            };
        }
    }};
}
