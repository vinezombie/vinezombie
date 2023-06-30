use super::Queue;
use crate::ircmsg::ServerMsg;

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

/// Closure-like types that can be run off of message streams.
///
/// This trait has a blanket implementation for [`FnMut`]s that
/// match the signature of `handle`.
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

/// Handlers that can generate asynchronous tasks from message streams.
///
/// Unlike [`Handler`], `HandlerAsync` has two methods:
/// one for handling messages, and one handling the completion of a task.
/// Both methods are provided a mutable reference to a collection `T`
/// to which tasks can be added, which will then be driven outside the handler.
pub trait HandlerAsync<T> {
    /// The value returned by tasks generated by this handler.
    type TaskValue;
    /// The type of value this handler yields on completion.
    type Value;
    /// The type of this handler's warnings.
    type Warning: std::fmt::Display;
    /// The type of this handler's errors.
    type Error: From<std::io::Error>;
    /// Handles one message.
    fn handle_msg(
        &mut self,
        msg: &ServerMsg<'static>,
        tasks: &mut T,
        queue: &mut Queue<'static>,
    ) -> HandlerResult<Self::Value, Self::Warning, Self::Error>;
    /// Handles a future yielding a value.
    fn handle_value(
        &mut self,
        value: Self::TaskValue,
        tasks: &mut T,
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

impl<T, H: Handler> HandlerAsync<T> for H {
    type TaskValue = std::convert::Infallible;

    type Value = H::Value;

    type Warning = H::Warning;

    type Error = H::Error;

    fn handle_msg(
        &mut self,
        msg: &ServerMsg<'static>,
        _: &mut T,
        queue: &mut Queue<'static>,
    ) -> HandlerResult<Self::Value, Self::Warning, Self::Error> {
        self.handle(msg, queue)
    }

    fn handle_value(
        &mut self,
        _: Self::TaskValue,
        _: &mut T,
        _: &mut Queue<'static>,
    ) -> HandlerResult<Self::Value, Self::Warning, Self::Error> {
        unimplemented!()
    }
}

/// Runs a [`Handler`] to completion off of synchronous I/O.
pub fn run_handler<H, V, W: std::fmt::Display, E: From<std::io::Error>>(
    conn: &mut impl super::conn::Connection,
    queue: &mut Queue<'static>,
    handler: &mut H,
) -> Result<V, E>
where
    H: Handler<Value = V, Warning = W, Error = E>,
{
    let mut buf = Vec::new();
    loop {
        let msg = queue.pop(|dur| conn.set_read_timeout(dur).unwrap());
        if let Some(msg) = msg {
            msg.send_to(conn.as_write(), &mut buf)?;
            continue;
        }
        match ServerMsg::read_owning_from(conn.as_bufread(), &mut buf) {
            Ok(msg) => match handler.handle(&msg, queue) {
                Ok(HandlerOk::Value(val)) => break Ok(val),
                #[cfg(feature = "tracing")]
                Ok(HandlerOk::Warning(w)) => tracing::warn!(target: "vinezombie", "{}", w),
                Ok(_) => (),
                Err(e) => break Err(e),
            },
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => (),
            Err(e) => break Err(e.into()),
        };
    }
}

/// Runs an [`HandlerAsync<JoinSet<T>>`][HandlerAsync] to completion off of async I/O.
///
/// This function can also be used to drive [`Handler`]s due to a blanket implementation
/// of `HandlerAsync` for them.
#[cfg(feature = "tokio")]
pub async fn run_handler_tokio<H, T: 'static, V, W: std::fmt::Display, E: From<std::io::Error>>(
    conn: &mut impl super::conn::ConnectionTokio,
    queue: &mut Queue<'static>,
    handler: &mut H,
) -> Result<V, E>
where
    H: HandlerAsync<tokio::task::JoinSet<T>, TaskValue = T, Value = V, Warning = W, Error = E>,
{
    let mut buf = Vec::new();
    let mut timeout = Option::<std::time::Duration>::None;
    let mut joinset = tokio::task::JoinSet::<T>::new();
    loop {
        let msg = queue.pop(|dur| timeout = dur);
        if let Some(msg) = msg {
            msg.send_to_tokio(conn.as_write(), &mut buf).await?;
            continue;
        }
        let read_fut = ServerMsg::read_owning_from_tokio(conn.as_bufread(), &mut buf);
        let result = tokio::select! {
            biased;
            msg = read_fut =>
                handler.handle_msg(&msg?, &mut joinset, queue),
            Some(Ok(task_value)) = joinset.join_next() =>
                handler.handle_value(task_value, &mut joinset, queue),
            () = tokio::time::sleep(timeout.unwrap_or_default()), if timeout.is_some() =>
                continue
        };
        match result? {
            HandlerOk::Value(val) => break Ok(val),
            #[cfg(feature = "tracing")]
            HandlerOk::Warning(w) => tracing::warn!(target: "vinezombie", "{}", w),
            _ => (),
        }
    }
}
