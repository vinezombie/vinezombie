#![doc = include_str!("../doc/rustdoc/client.md")]

pub mod auth;
pub mod cap;
pub mod conn;
mod handler;
pub mod nick;
mod queue;
pub mod register;
mod sink;
#[cfg(feature = "tls")]
pub mod tls;

pub use {handler::*, queue::*, sink::*};

use crate::consts::cmd::{PING, PONG};
use crate::ircmsg::{ClientMsg, ServerMsg};

/// Returns a message in reply to a server ping.
pub fn pong(msg: &ServerMsg<'_>) -> Option<ClientMsg<'static>> {
    (msg.kind == PING).then(|| {
        let mut ret = ClientMsg::new_cmd(PONG);
        ret.args = msg.args.clone().owning();
        ret
    })
}

/// A lient connection.
#[derive(Debug, Default)]
pub struct Client<C, A = ()> {
    /// The connection to the IRC server.
    conn: C,
    /// A strategy for updating the queue from incoming messages.
    adjuster: A,
    /// A message queue for rate-limiting.
    queue: queue::Queue<'static>,
    /// A buffer that is used internally for message I/O.
    buf: Vec<u8>,
    // TODO: Read timeouts should go here.
}

impl<C, A> Client<C, A> {
    /// Extracts the connection from `self`, allowing it to be used elsewhere.
    pub fn take_conn(self) -> C {
        self.conn
    }
    /// Uses the provided [`Adjuster`][adjuster::Adjuster] for `self`.
    pub fn with_adjuster<A2: adjuster::Adjuster>(self, adjuster: A2) -> Client<C, A2> {
        let Self { conn, queue, buf, .. } = self;
        Client { conn, queue, adjuster, buf }
    }
    /// Uses the provided connection for `self`.
    pub fn with_conn<C2>(self, conn: C2) -> Client<C2, A> {
        let Self { queue, adjuster, buf, .. } = self;
        Client { conn, queue, adjuster, buf }
    }
    /// Returns a shared reference to the internal [`Adjuster`][adjuster::Adjuster].
    pub fn adjuster(&self) -> &A {
        &self.adjuster
    }
    /// Returns a shared reference to the internal [`Queue`].
    pub fn queue(&self) -> &Queue<'static> {
        &self.queue
    }
    /// Enqueues a [`ClientMsg`] to be sent.
    pub fn enqueue_one(&mut self, msg: ClientMsg<'static>) {
        self.queue.push(msg);
    }
    /// Changes the queue rate limit as [`Queue::set_rate_limit`].
    pub fn set_rate_limit(&mut self, delay: std::time::Duration, burst: u32) {
        self.queue.set_rate_limit(delay, burst);
    }
}

impl<C, A: adjuster::Adjuster> Client<C, A> {
    /// Creates a new `Client` from the provided connection
    /// [queue adjustment strategy][adjuster::Adjuster].
    pub fn new_with_adjuster(conn: C, adjuster: A) -> Self {
        Client { conn, queue: Queue::new(), adjuster, buf: Vec::new() }
    }
}

impl<C, A: adjuster::Adjuster + Default> Client<C, A> {
    /// Creates a new `Client` from the provided connection.
    pub fn new(conn: C) -> Self {
        Self::new_with_adjuster(conn, A::default())
    }
}

// Implementations of other Client methods can be found in `conn`,
// specifically the submodules depending on I/O flavor.
