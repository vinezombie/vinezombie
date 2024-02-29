use super::QueueEditGuard;
use crate::ircmsg::ClientMsg;

/// Final destinations for [`ClientMsg`]s.
///
/// A `ClientMsgSink` has two properpies: it can infallibly accept `ClientMsg`s,
/// and is has a notion of being mutably borrowed in a form that is also a `ClientMsgSink`
/// (which will usually but not neccessarily be `&mut Self`).
///
/// Most of the handler functions accept one of these instead of returning `Vec`s
/// full of client messages to send.
pub trait ClientMsgSink<'a> {
    /// Sends a [`ClientMsg`].
    fn send(&mut self, msg: ClientMsg<'a>);
    /// The borrowed form of `self`, usually `&mut Self`
    type Borrowed<'b>: ClientMsgSink<'a>
    where
        Self: 'b;
    /// Mutably borrows self as a `ClientMsgSink`.
    fn borrow_mut(&mut self) -> Self::Borrowed<'_>;
}

impl<'a, F: FnMut(ClientMsg<'a>)> ClientMsgSink<'a> for F {
    fn send(&mut self, msg: ClientMsg<'a>) {
        self(msg);
    }

    type Borrowed<'b> = &'b mut F where F: 'b;

    fn borrow_mut(&mut self) -> Self::Borrowed<'_> {
        self
    }
}

impl<'a> ClientMsgSink<'static> for &mut QueueEditGuard<'a> {
    fn send(&mut self, msg: ClientMsg<'static>) {
        self.push(msg);
    }

    type Borrowed<'b> = &'b mut QueueEditGuard<'a> where Self: 'b;

    fn borrow_mut(&mut self) -> Self::Borrowed<'_> {
        self
    }
}
