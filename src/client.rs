#![doc = include_str!("../doc/rustdoc/client.md")]

pub mod auth;
pub mod cap;
pub mod conn;
mod handler;
pub mod handlers;
pub mod nick;
mod queue;
pub mod register;
mod sink;
#[cfg(feature = "tls")]
pub mod tls;

pub use {handler::*, queue::*, sink::*};

use self::channel::ChannelSpec;

/// A client connection.
#[derive(Default)]
pub struct Client<C, A = ()> {
    /// The connection to the IRC server.
    conn: C,
    /// A strategy for updating the queue from incoming messages.
    adjuster: A,
    /// A message queue for rate-limiting.
    queue: Box<queue::Queue>,
    /// A buffer that is used internally for inbound message I/O.
    buf_i: Vec<u8>,
    /// A buffer that is used internally for outbound message I/O.
    buf_o: Vec<u8>,
    /// Collection of handlers.
    handlers: Handlers,
    /// Limit on how long reading one message can take.
    timeout: Box<conn::time::TimeLimits>,
}

/// Creates a new [`Client`] out of a connection with sensible default types.
///
/// Note that connection registration will still likely need to happen after this.
pub fn new_client<C>(conn: C) -> Client<C, impl adjuster::Adjuster> {
    Client::new_with_adjuster(conn, ())
}

impl<C, A> Client<C, A> {
    /// Extracts the connection from `self`, allowing it to be used elsewhere.
    pub fn take_conn(self) -> C {
        self.conn
    }
    /// Uses the provided connection for `self`.
    ///
    /// This operation resets `self`'s timeouts,
    /// as this function has no knowledge of existing timeouts set on the connection.
    pub fn with_conn<C2>(self, conn: C2) -> Client<C2, A> {
        let Self { queue, adjuster, buf_i, buf_o, handlers, mut timeout, .. } = self;
        *timeout = Default::default();
        Client { conn, queue, adjuster, buf_i, buf_o, timeout, handlers }
    }
    /// Uses the provided [`Adjuster`][adjuster::Adjuster] for `self`.
    pub fn with_adjuster<A2: adjuster::Adjuster>(self, adjuster: A2) -> Client<C, A2> {
        let Self { conn, queue, buf_i, buf_o, handlers, timeout, .. } = self;
        Client { conn, queue, adjuster, buf_i, buf_o, handlers, timeout }
    }
    /// Returns a shared reference to the internal [`Adjuster`][adjuster::Adjuster].
    pub fn adjuster(&self) -> &A {
        &self.adjuster
    }
    /// Returns a mutable reference to the internal [`Adjuster`][adjuster::Adjuster]>
    pub fn adjuster_mut(&mut self) -> &mut A {
        &mut self.adjuster
    }
    /// Returns a shared reference to the internal [`Queue`].
    pub fn queue(&self) -> &Queue {
        &self.queue
    }
    /// Returns a mutable reference to the internal [`Queue`].
    ///
    /// This reference should not be used to alter the queue's contents, only its settings.
    pub fn queue_mut(&mut self) -> &mut Queue {
        &mut self.queue
    }
    /// Changes the upper limit on how long an I/O operation may take to receive one message.
    /// A timeout of `None` means no limit.
    ///
    /// This upper limit should be long, on the order of tens of seconds.
    pub fn set_read_timeout(&mut self, timeout: Option<std::time::Duration>) -> &mut Self {
        self.timeout.set_read_timeout(timeout);
        self
    }
    /// Changes the upper limit on how long an I/O operation may take to send any data.
    /// A timeout of `None` means no limit.
    ///
    /// This upper limit should be fairly short, on the order of a few seconds.
    pub fn set_write_timeout(&mut self, timeout: Option<std::time::Duration>) -> &mut Self {
        self.timeout.set_write_timeout(timeout);
        self
    }

    /// Adds a handler. Creates a new channel using the provided [`ChannelSpec`].
    ///
    /// Returns the handler id and the receiver half of the channel.
    pub fn add<T, M: MakeHandler<T>, S: ChannelSpec>(
        &mut self,
        chanspec: &S,
        make_handler: &M,
        value: T,
    ) -> Result<(usize, M::Receiver<S>), M::Error> {
        let (send, recv) = M::make_channel(chanspec);
        Ok((self.add_with_sender(send, make_handler, value)?, recv))
    }

    /// Adds a handler using an existing channel.
    ///
    /// Returns the handler id.
    pub fn add_with_sender<T, M: MakeHandler<T>>(
        &mut self,
        sender: std::sync::Arc<dyn channel::Sender<Value = M::Value>>,
        make_handler: &M,
        value: T,
    ) -> Result<usize, M::Error> {
        let handler = make_handler.make_handler(self.queue.edit(), value)?;
        Ok(self.handlers.add(handler, sender))
    }
}

impl<C, A: adjuster::Adjuster> Client<C, A> {
    /// Creates a new `Client` from the provided
    /// connection and [queue adjustment strategy][adjuster::Adjuster].
    pub fn new_with_adjuster(conn: C, adjuster: A) -> Self {
        Self::new_with_adjuster_and_queue(conn, adjuster, Queue::new())
    }
    /// Creates a new `Client` from the provided
    /// connection, [queue adjustment strategy][adjuster::Adjuster], and [`Queue`].
    pub fn new_with_adjuster_and_queue(conn: C, adjuster: A, queue: Queue) -> Self {
        Client {
            conn,
            queue: Box::new(queue),
            adjuster,
            buf_i: Vec::new(),
            buf_o: Vec::new(),
            handlers: Handlers::default(),
            timeout: Box::default(),
        }
    }
}

impl<C, A: adjuster::Adjuster + Default> Client<C, A> {
    /// Creates a new `Client` from the provided connection.
    pub fn new(conn: C) -> Self {
        Self::new_with_adjuster(conn, A::default())
    }
    /// Creates a new `Client` from the provided connection and [`Queue`].
    pub fn new_with_queue(conn: C, queue: Queue) -> Self {
        Self::new_with_adjuster_and_queue(conn, A::default(), queue)
    }
}

impl<C: Default, A: adjuster::Adjuster + Default> From<Queue> for Client<C, A> {
    fn from(queue: Queue) -> Self {
        Self::new_with_queue(C::default(), queue)
    }
}

// Implementations of other Client methods can be found in `conn`,
// specifically the submodules depending on I/O flavor.
