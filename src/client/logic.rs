use super::{
    channel::{ChannelSpec, Sender},
    conn::TimeLimits,
    state::ClientStateKey,
    Handlers, MakeHandler, Queue,
};

/// The parts of client logic that are not dependent on the type of connection or channel spec.
#[derive(Default)]
pub struct ClientLogic {
    /// State used for I/O.
    pub(super) timeout: TimeLimits,
    /// A message queue for rate-limiting.
    pub(super) queue: Queue,
    /// Shared state.
    pub(super) state: ClientState,
    /// Collection of handlers.
    pub(super) handlers: Handlers,
}

impl ClientLogic {
    /// Creates a new `ClientLogic`.
    pub fn new() -> ClientLogic {
        ClientLogic::default()
    }
    /// Uses the provided [`Queue`] in `self`.
    pub fn with_queue(self, queue: Queue) -> Self {
        Self { queue, ..self }
    }
    /// Uses the provided [`ClientState`] in `self`.
    pub fn with_state(self, state: ClientState) -> Self {
        Self { state, ..self }
    }
    /// Sets the upper limit on how long an I/O operation may take to receive one message.
    ///
    /// This is a convenience method for use during construction.
    /// This upper limit should be long, on the order of a few half-minutes.
    pub fn with_read_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.timeout.set_read_timeout(Some(timeout));
        self
    }
    /// Sets the upper limit on how long an I/O operation may take to send any data.
    ///
    /// This is a convenience method for use during construction.
    /// This upper limit should be fairly short, on the order of a few seconds.
    pub fn with_write_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.timeout.set_write_timeout(Some(timeout));
        self
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
    /// Sets the upper limit on how long an I/O operation may take to receive one message.
    /// A timeout of `None` means no limit.
    ///
    /// This upper limit should be long, on the order of a few half-minutes.
    pub fn set_read_timeout(&mut self, timeout: Option<std::time::Duration>) {
        self.timeout.set_read_timeout(timeout);
    }
    /// Sets the upper limit on how long an I/O operation may take to send any data.
    /// A timeout of `None` means no limit.
    ///
    /// This upper limit should be fairly short, on the order of a few seconds.
    pub fn set_write_timeout(&mut self, timeout: Option<std::time::Duration>) {
        self.timeout.set_write_timeout(timeout);
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
        sender: Box<dyn Sender<Value = M::Value> + Send>,
        make_handler: M,
        value: T,
    ) -> Result<usize, M::Error> {
        let handler = make_handler.make_handler(&self.state, self.queue.edit(), value)?;
        Ok(self.handlers.add(handler, sender))
    }

    /// Resets state to when the connection was just opened.
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

    /// Returns `true` if the client has handlers or queued messages.
    pub fn needs_run(&self) -> bool {
        !self.handlers.is_empty() || !self.queue.is_empty()
    }

    /// Processes one message from the server.
    pub(super) fn run_once(&mut self, msg: &crate::ircmsg::ServerMsg<'_>) -> usize {
        self.queue.adjust(msg);
        self.handlers.handle(msg, &mut self.state, &mut self.queue)
    }
}

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
    pub fn get<K: ClientStateKey>(&self) -> Option<&K::Value> {
        self.state.get(&K::default().type_id()).and_then(|v| v.1.downcast_ref())
    }
    /// Gets a mutable reference to the state denoted by `K`, if any.
    pub fn get_mut<K: ClientStateKey>(&mut self) -> Option<&mut K::Value> {
        self.state.get_mut(&K::default().type_id()).and_then(|v| v.1.downcast_mut())
    }
    /// Sets the state denoted by `K` to `value`.
    ///
    /// This should be called infrequently. Prefer [`ClientState::get_mut`] for most updates.
    pub fn insert<K: ClientStateKey>(&mut self, value: K::Value) {
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
