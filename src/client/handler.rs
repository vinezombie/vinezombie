use crate::ircmsg::ServerMsg;

use super::Queue;

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

/// Closure-like types that can be run directly off of connections.
pub trait Handler {
    /// The type of value this handler yields on completion.
    type Value;
    /// The type of this handler's warnings.
    type Warning: std::fmt::Display;
    /// The type of this handler's errors.
    type Error: From<std::io::Error>;
    /// Handles one message.
    fn handle(
        &mut self,
        msg: &ServerMsg<'static>,
        queue: &mut Queue<'static>,
    ) -> HandlerResult<Self::Value, Self::Warning, Self::Error>;
}

impl<
        T,
        W: std::fmt::Display,
        E: From<std::io::Error>,
        F: FnMut(&ServerMsg<'static>, &mut Queue<'static>) -> HandlerResult<T, W, E>,
    > Handler for F
{
    type Value = T;
    type Warning = W;
    type Error = E;
    fn handle(
        &mut self,
        msg: &ServerMsg<'static>,
        queue: &mut Queue<'static>,
    ) -> HandlerResult<Self::Value, Self::Warning, Self::Error> {
        self(msg, queue)
    }
}

/// Runs a handler function to completion.
pub fn run_handler<H, T, W: std::fmt::Display, E: From<std::io::Error>>(
    conn: &mut impl super::conn::Connection,
    queue: &mut Queue<'static>,
    handler: &mut H,
) -> Result<T, E>
where
    H: Handler<Value = T, Warning = W, Error = E>,
{
    let mut buf = Vec::new();
    loop {
        let msg = queue.pop(|dur| conn.set_read_timeout(dur).unwrap());
        if let Some(msg) = msg {
            if let Err(e) = msg.send_to(conn.as_write(), &mut buf) {
                break Err(e.into());
            }
            continue;
        }
        match ServerMsg::read_owning_from(conn.as_bufread(), &mut buf) {
            Ok(msg) => match handler.handle(&msg, queue) {
                Ok(HandlerOk::Value(val)) => break Ok(val),
                Ok(_) => (),
                Err(e) => break Err(e),
            },
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => (),
            Err(e) => break Err(e.into()),
        };
    }
}
