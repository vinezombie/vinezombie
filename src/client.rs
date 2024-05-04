#![doc = include_str!("../doc/rustdoc/client.md")]

pub mod auth;
pub mod cap;
pub mod conn;
mod handler;
pub mod handlers;
mod logic;
pub mod nick;
pub mod queue;
pub mod register;
mod sink;
pub mod state;
#[cfg(feature = "tls")]
pub mod tls;

pub use {handler::*, logic::*, sink::*};

use self::{channel::ChannelSpec, queue::Queue};

/// A client connection.
#[derive(Default)]
pub struct Client<C, S> {
    /// The connection to the IRC server.
    conn: conn::MsgIo<C>,
    /// The [`ChannelSpec`] for creating now channels.
    spec: S,
    /// This client's internal logic.
    logic: Box<ClientLogic>,
    // /// Logic for handling read timeouts.
    // #[allow(clippy::type_complexity)]
    // on_timeout: Option<Box<dyn FnMut(&mut ClientLogic) -> std::ops::ControlFlow<()> + Send>>,
}

impl<C, S: ChannelSpec> Client<C, S> {
    /// Creates a new `Client` from the provided connection.
    pub fn new(conn: C, spec: S) -> Self {
        Self::new_with_logic(conn, spec, ClientLogic::new())
    }
    #[deprecated = "Removed in 0.4; use `Client::new_with_logic`."]
    /// Creates a new `Client` from the provided connection and [`Queue`].
    pub fn new_with_queue(conn: C, spec: S, queue: Queue) -> Self {
        Self::new_with_logic(conn, spec, ClientLogic::new().with_queue(queue))
    }
    /// Creates a new `Client` from the provided connection and [`Queue`].
    pub fn new_with_logic(conn: C, spec: S, logic: ClientLogic) -> Self {
        Client {
            conn: conn::MsgIo::new(conn),
            spec,
            logic: Box::new(logic),
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
        self.conn.conn
    }
    /// Uses the provided connection for `self`.
    ///
    /// This connection does not change any of [`Client`]s state aside from
    /// requiring an update of the connection's IO timeouts.
    /// Additionally use [`reset`][Client::reset] if you want to reset the state.
    pub fn with_conn<C2>(self, conn: C2) -> Client<C2, S> {
        let Self { conn: old, spec, mut logic } = self;
        logic.timeout.require_update();
        let conn = conn::MsgIo { conn, buf_i: old.buf_i, buf_o: old.buf_o };
        Client { conn, logic, spec }
    }
    /// Uses the provided [`ChannelSpec`] for `self`.
    /// This changes the type of channels returned by [`add`][Client::add].
    pub fn with_spec<S2: ChannelSpec>(self, spec: S2) -> Client<C, S2> {
        let Self { conn, logic, .. } = self;
        Client { conn, spec, logic }
    }
    /// Returns a shared reference to the internal [`Queue`].
    pub fn queue(&self) -> &Queue {
        self.logic.queue()
    }
    /// Returns a mutable reference to the internal [`Queue`].
    ///
    /// Removing items from the queue may confuse handlers.
    pub fn queue_mut(&mut self) -> &mut Queue {
        self.logic.queue_mut()
    }
    /// Returns a shared reference to the internal [shared state][ClientState].
    pub fn state(&self) -> &ClientState {
        self.logic.state()
    }
    /// Returns a mutable reference to the internal [shared state][ClientState].
    pub fn state_mut(&mut self) -> &mut ClientState {
        self.logic.state_mut()
    }
    /// Changes the upper limit on how long an I/O operation may take to receive one message.
    /// A timeout of `None` means no limit.
    ///
    /// This upper limit should be long, on the order of tens of seconds.
    pub fn set_read_timeout(&mut self, timeout: Option<std::time::Duration>) -> &mut Self {
        self.logic.set_read_timeout(timeout);
        self
    }
    /// Changes the upper limit on how long an I/O operation may take to send any data.
    /// A timeout of `None` means no limit.
    ///
    /// This upper limit should be fairly short, on the order of a few seconds.
    pub fn set_write_timeout(&mut self, timeout: Option<std::time::Duration>) -> &mut Self {
        self.logic.set_write_timeout(timeout);
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
        self.logic.add_with_spec(chanspec, make_handler, value)
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
        self.logic.add_with_sender(sender, make_handler, value)
    }

    /// Resets client state to when the connection was just opened.
    ///
    /// Cancels all handlers, removes all [shared state][ClientState],
    /// and resets the [queue][Queue] including removing the [queue's labeler][Queue::use_labeler].
    /// Does not reset any state that is considered configuration,
    /// such as what the queue's rate limits are.
    pub fn reset(&mut self) {
        self.logic.reset();
    }

    /// Uses the provided connection instead of the current one
    /// and resets the client state as [`reset`][Client::reset].
    /// Returns the old connection.
    ///
    /// This results in a fresh [`Client`] ready to perform connection registration again.
    pub fn reset_with_conn(&mut self, conn: C) -> C {
        let retval = std::mem::replace(&mut self.conn.conn, conn);
        self.logic.timeout.require_update();
        self.reset();
        retval
    }
    /// Returns `true` if the client has handlers or queued messages.
    pub fn needs_run(&self) -> bool {
        self.logic.needs_run()
    }
}
