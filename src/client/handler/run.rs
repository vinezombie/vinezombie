use super::{DelayQueue, HandlerAsync, HandlerOk};
use crate::{
    client::{conn::Connection, Queue},
    ircmsg::ServerMsg,
    util::option_union_with,
};

/// Runs a [`HandlerAsync<DelayQueue<T>>`][HandlerAsync] to completion off of synchronous I/O.
///
/// This function can also be used to drive [`Handler`][crate::client::Handler]s
/// due to a blanket implementation of `HandlerAsync` for them.
pub fn run_handler<H, T: 'static, V, W: std::fmt::Display, E: From<std::io::Error>>(
    conn: &mut impl Connection,
    queue: &mut Queue<'static>,
    handler: &mut H,
) -> Result<V, E>
where
    H: HandlerAsync<super::DelayQueue<T>, TaskValue = T, Value = V, Warning = W, Error = E>,
{
    let mut delayqueue = DelayQueue::new();
    let mut buf = Vec::with_capacity(512);
    loop {
        let next_timeout = delayqueue.next_timeout();
        let result = if next_timeout.is_some_and(|dur| dur.is_zero()) {
            let Some(value) = delayqueue.pop() else {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "connection timed out",
                )
                .into());
            };
            handler.handle_value(value, &mut delayqueue, queue)
        } else {
            let msg = queue.pop(|dur| {
                conn.set_read_timeout(option_union_with(dur, next_timeout, std::cmp::min)).unwrap();
            });
            if let Some(msg) = msg {
                msg.send_to(conn.as_write(), &mut buf)?;
                continue;
            }
            match ServerMsg::read_borrowing_from(conn.as_bufread(), &mut buf) {
                Ok(msg) => {
                    let result = handler.handle_msg(&msg, &mut delayqueue, queue);
                    buf.clear();
                    result
                }
                Err(e) if e.kind() == std::io::ErrorKind::TimedOut => continue,
                Err(e) => break Err(e.into()),
            }
        };
        match result {
            Ok(HandlerOk::Value(val)) => break Ok(val),
            #[cfg(feature = "tracing")]
            Ok(HandlerOk::Warning(w)) => tracing::warn!(target: "vinezombie", "{}", w),
            Ok(_) => (),
            Err(e) => break Err(e),
        }
    }
}

/// Runs a [`HandlerAsync<JoinSet<T>>`][HandlerAsync] to completion off of async I/O.
///
/// This function can also be used to drive [`Handler`][crate::client::Handler]s
/// due to a blanket implementation of `HandlerAsync` for them.
#[cfg(feature = "tokio")]
pub async fn run_handler_tokio<H, T: 'static, V, W: std::fmt::Display, E: From<std::io::Error>>(
    conn: &mut impl crate::client::conn::ConnectionTokio,
    queue: &mut Queue<'static>,
    handler: &mut H,
) -> Result<V, E>
where
    H: HandlerAsync<tokio::task::JoinSet<T>, TaskValue = T, Value = V, Warning = W, Error = E>,
{
    let mut buf = Vec::with_capacity(512);
    let mut timeout = Option::<std::time::Duration>::None;
    let mut joinset = tokio::task::JoinSet::<T>::new();
    loop {
        let msg = queue.pop(|dur| timeout = dur);
        if let Some(msg) = msg {
            msg.send_to_tokio(conn.as_write(), &mut buf).await?;
            continue;
        }
        let fut = RunHandlerTokioFuture {
            conn,
            buf: &mut buf,
            joinset: &mut joinset,
            timeout,
            handler,
            queue,
        };
        let Some(result) = fut.await else { continue };
        buf.clear();
        match result? {
            HandlerOk::Value(val) => break Ok(val),
            #[cfg(feature = "tracing")]
            HandlerOk::Warning(w) => tracing::warn!(target: "vinezombie", "{}", w),
            _ => (),
        }
    }
}

#[cfg(feature = "tokio")]
struct RunHandlerTokioFuture<'a, C, T, H> {
    conn: &'a mut C,
    buf: &'a mut Vec<u8>,
    joinset: &'a mut tokio::task::JoinSet<T>,
    timeout: Option<std::time::Duration>,
    handler: &'a mut H,
    queue: &'a mut Queue<'static>,
}

#[cfg(feature = "tokio")]
impl<'a, C, T, H> std::future::Future for RunHandlerTokioFuture<'a, C, T, H>
where
    C: crate::client::conn::ConnectionTokio,
    T: 'static,
    H: HandlerAsync<tokio::task::JoinSet<T>, TaskValue = T>,
    H::Error: From<std::io::Error>,
{
    type Output = Option<super::HandlerResult<H::Value, H::Warning, H::Error>>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        use std::task::Poll;
        let this = self.get_mut();
        if let Some(sleep) = &mut this.timeout {
            let sleep = tokio::time::sleep(*sleep);
            tokio::pin!(sleep);
            if let Poll::Ready(()) = sleep.poll(cx) {
                return Poll::Ready(None);
            }
        }
        let read_fut = ServerMsg::read_borrowing_from_tokio(this.conn.as_bufread(), this.buf);
        tokio::pin!(read_fut);
        if let Poll::Ready(result) = read_fut.poll(cx) {
            let retval = match result {
                Ok(msg) => {
                    this.handler.handle_msg(&msg, this.joinset, this.queue)
                    // this.buf gets cleared later after the await.
                }
                Err(e) => Err(e.into()),
            };
            return Poll::Ready(Some(retval));
        }
        if let Poll::Ready(Some(Ok(task_value))) = this.joinset.poll_join_next(cx) {
            let retval = this.handler.handle_value(task_value, this.joinset, this.queue);
            return Poll::Ready(Some(retval));
        }
        Poll::Pending
    }
}
