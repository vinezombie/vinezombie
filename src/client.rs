#![doc = include_str!("../doc/rustdoc/client.md")]

pub mod auth;
pub mod cap;
pub mod conn;
mod handler;
pub mod handlers;
pub mod nick;
pub mod queue;
pub mod register;
mod sink;
pub mod state;
#[cfg(feature = "tls")]
pub mod tls;

pub use {handler::*, sink::*};

use self::{channel::ChannelSpec, queue::Queue};

/// A client connection.
#[derive(Default)]
pub struct Client<C, S> {
    /// The connection to the IRC server.
    conn: C,
    /// The [`ChannelSpec`] for creating now channels.
    spec: S,
    /// A message queue for rate-limiting.
    queue: Box<Queue>,
    /// A buffer that is used internally for inbound message I/O.
    buf_i: Vec<u8>,
    /// A buffer that is used internally for outbound message I/O.
    buf_o: Vec<u8>,
    /// Collection of handlers.
    handlers: Handlers,
    /// Limit on how long reading one message can take.
    timeout: Box<conn::TimeLimits>,
    /// Shared state.
    state: ClientState,
}

impl<C, S: ChannelSpec> Client<C, S> {
    /// Creates a new `Client` from the provided connection.
    pub fn new(conn: C, spec: S) -> Self {
        Self::new_with_queue(conn, spec, Queue::new())
    }
    /// Creates a new `Client` from the provided connection and [`Queue`].
    pub fn new_with_queue(conn: C, spec: S, queue: Queue) -> Self {
        Client {
            conn,
            spec,
            queue: Box::new(queue),
            buf_i: Vec::new(),
            buf_o: Vec::new(),
            handlers: Handlers::default(),
            timeout: Box::default(),
            state: ClientState::new(),
        }
    }
    /// Adds a handler. Creates a new channel using the internal [`ChannelSpec`].
    ///
    /// Returns the handler id and the receiver half of the channel.
    pub fn add<T, M: MakeHandler<T>>(
        &mut self,
        make_handler: M,
        value: T,
    ) -> Result<(usize, M::Receiver<S>), M::Error> {
        let (send, recv) = M::make_channel(&self.spec);
        Ok((self.add_with_sender(send, make_handler, value)?, recv))
    }
}

impl<C, S> Client<C, S> {
    /// Extracts the connection from `self`, allowing it to be used elsewhere.
    pub fn take_conn(self) -> C {
        self.conn
    }
    /// Uses the provided connection for `self`.
    ///
    /// This connection does not change any of [`Client`]s state aside from
    /// requiring an update of the connection's IO timeouts.
    /// Additionally use [`reset`][Client::reset] if you want to reset the state.
    pub fn with_conn<C2>(self, conn: C2) -> Client<C2, S> {
        let Self { spec, queue, buf_i, buf_o, handlers, mut timeout, state, .. } = self;
        timeout.require_update();
        Client { conn, spec, queue, buf_i, buf_o, timeout, handlers, state }
    }
    /// Uses the provided [`ChannelSpec`] for `self`.
    /// This changes the type of channels returned by [`add`][Client::add].
    pub fn with_spec<S2: ChannelSpec>(self, spec: S2) -> Client<C, S2> {
        let Self { conn, queue, buf_i, buf_o, handlers, timeout, state, .. } = self;
        Client { conn, spec, queue, buf_i, buf_o, timeout, handlers, state }
    }
    /// Returns a shared reference to the internal [`Queue`].
    pub fn queue(&self) -> &Queue {
        &self.queue
    }
    /// Returns a mutable reference to the internal [`Queue`].
    ///
    /// Removing items from the queue may confuse handlers.
    pub fn queue_mut(&mut self) -> &mut Queue {
        &mut self.queue
    }
    /// Returns a shared reference to the internal [shared state][ClientState].
    pub fn state(&self) -> &ClientState {
        &self.state
    }
    /// Returns a mutable reference to the internal [shared state][ClientState].
    pub fn state_mut(&mut self) -> &mut ClientState {
        &mut self.state
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
    pub fn add_with_spec<T, M: MakeHandler<T>, S2: ChannelSpec>(
        &mut self,
        chanspec: &S2,
        make_handler: M,
        value: T,
    ) -> Result<(usize, M::Receiver<S2>), M::Error> {
        let (send, recv) = M::make_channel(chanspec);
        Ok((self.add_with_sender(send, make_handler, value)?, recv))
    }

    /// Adds a handler using an existing channel.
    ///
    /// Returns the handler id.
    pub fn add_with_sender<T, M: MakeHandler<T>>(
        &mut self,
        sender: Box<dyn channel::Sender<Value = M::Value> + Send>,
        make_handler: M,
        value: T,
    ) -> Result<usize, M::Error> {
        let handler = make_handler.make_handler(&self.state, self.queue.edit(), value)?;
        Ok(self.handlers.add(handler, sender))
    }

    /// Resets client state to when the connection was just opened.
    ///
    /// Cancels all handlers, removes all [shared state][ClientState],
    /// and resets the [queue][Queue] including removing the [queue's labeler][Queue::use_labeler].
    /// Does not reset any state that is considered configuration,
    /// such as what the queue's rate limits are.
    pub fn reset(&mut self) {
        self.handlers.cancel();
        self.queue.reset();
        self.state.clear();
    }

    /// Uses the provided connection instead of the current one
    /// and resets the client state as [`reset`][Client::reset].
    /// Returns the old connection.
    ///
    /// This results in a fresh [`Client`] ready to perform connection registration again.
    pub fn reset_with_conn(&mut self, conn: C) -> C {
        let retval = std::mem::replace(&mut self.conn, conn);
        self.timeout.require_update();
        self.reset();
        retval
    }
    /// Returns `true` if the client has handlers or queued messages.
    pub fn needs_run(&self) -> bool {
        !self.handlers.is_empty() || !self.queue.is_empty()
    }
}

// Implementations of other Client methods can be found in `conn`,
// specifically the submodules depending on I/O flavor.

/// A collection of heterogenous client state shared between handlers.
///
/// There are many pieces of client state that need to be shared between handlers,
/// such as the client's source string for accurate message length calculations.
/// This type facilitates that in an extensible manner, allowing handlers to
/// add new elements of state at runtime.
///
/// This type intentionally offers no way for state to be removed.
pub struct ClientState {
    state: crate::util::FlatMap<(std::any::TypeId, Box<dyn std::any::Any + Send + Sync>)>,
}

impl ClientState {
    /// Returns a new, empty `ClientState`.
    pub const fn new() -> ClientState {
        ClientState { state: crate::util::FlatMap::new() }
    }
    /// Gets a shared reference to the state denoted by `K`, if any.
    pub fn get<K: state::ClientStateKey>(&self) -> Option<&K::Value> {
        self.state.get(&K::default().type_id()).and_then(|v| v.1.downcast_ref())
    }
    /// Gets a mutable reference to the state denoted by `K`, if any.
    pub fn get_mut<K: state::ClientStateKey>(&mut self) -> Option<&mut K::Value> {
        self.state.get_mut(&K::default().type_id()).and_then(|v| v.1.downcast_mut())
    }
    /// Sets the state denoted by `K` to `value`.
    ///
    /// This should be called infrequently. Prefer [`ClientState::get_mut`] for most updates.
    pub fn insert<K: state::ClientStateKey>(&mut self, value: K::Value) {
        self.state.edit().insert((K::default().type_id(), Box::new(value)));
    }
    /// Clears all state.
    pub(super) fn clear(&mut self) {
        self.state.clear();
    }
}

impl Default for ClientState {
    fn default() -> Self {
        Self::new()
    }
}
