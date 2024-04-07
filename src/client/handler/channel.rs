//! Abstractions for returning data from handlers.
//!
//! No relation to IRC channels.

/// Send halves of non-blocking channels
pub trait Sender {
    /// The type of values consumed by this channel.
    type Value;

    /// Attempts to send a value over the channel.
    ///
    /// This function must never block, as if may be called from async contexts.
    fn send(&mut self, value: Self::Value) -> SendCont;

    /// Returns whether attempting to send a value may succeed.
    ///
    /// A return value of `false` means a future send operation is guaranteed to fail.
    /// A return value of `true` means a future send operation may or may not succeed.
    fn may_send(&self) -> bool {
        true
    }
}

/// The outcome of attempting to send a message via a [`Sender`].
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[repr(u8)]
pub enum SendCont {
    /// The message was not sent and the channel accepts no further messages.
    Closed,
    /// The message was sent, but the channel accepts no further messages.
    SentClosed,
    /// The message was sent.
    Sent,
}

/// The sender half of channel as provided to a handler.
pub struct SenderRef<'a, T> {
    pub(super) sender: &'a mut dyn Sender<Value = T>,
    pub(super) flag: &'a mut bool,
}

impl<'a, T> SenderRef<'a, T> {
    /// Sends one value to the underlying channel.
    ///
    /// Returns `true` if the value sent successfully, otherwise returns `false`.
    /// This return value can often be safely ignored.
    pub fn send(&mut self, value: T) -> bool {
        let result = self.sender.send(value);
        let success = !matches!(result, SendCont::Closed);
        *self.flag |= success;
        success
    }
    /// Returns `false` if a later send operation is guaranteed to fail.
    pub fn may_send(&self) -> bool {
        self.sender.may_send()
    }
}

/// A [`Sender`] that is always closed.
#[derive(Clone, Copy, Debug, Default)]
pub struct ClosedSender<T>(std::marker::PhantomData<fn(T)>);

impl<T> Sender for ClosedSender<T> {
    type Value = T;

    fn send(&mut self, _value: T) -> SendCont {
        SendCont::Closed
    }

    fn may_send(&self) -> bool {
        false
    }
}

impl<T: 'static> Sender for std::sync::mpsc::Sender<T> {
    type Value = T;

    fn send(&mut self, value: T) -> SendCont {
        if std::sync::mpsc::Sender::send(&*self, value).is_ok() {
            SendCont::Sent
        } else {
            SendCont::Closed
        }
    }
}

#[cfg(feature = "tokio")]
impl<T> Sender for Option<tokio::sync::oneshot::Sender<T>> {
    type Value = T;

    fn send(&mut self, value: Self::Value) -> SendCont {
        if let Some(sender) = self.take() {
            if sender.send(value).is_ok() {
                return SendCont::SentClosed;
            }
        }
        SendCont::Closed
    }
}

#[cfg(feature = "tokio")]
impl<T: 'static> Sender for tokio::sync::mpsc::UnboundedSender<T> {
    type Value = T;

    fn send(&mut self, value: T) -> SendCont {
        if tokio::sync::mpsc::UnboundedSender::send(&*self, value).is_ok() {
            SendCont::Sent
        } else {
            SendCont::Closed
        }
    }
}

#[cfg(feature = "tokio")]
impl<T: 'static> Sender for tokio::sync::mpsc::WeakUnboundedSender<T> {
    type Value = T;

    fn send(&mut self, value: T) -> SendCont {
        if let Some(sender) = self.upgrade() {
            if sender.send(value).is_ok() {
                return SendCont::Sent;
            }
        }
        SendCont::Closed
    }
}

/// Specifications for channel types.
///
/// All of the type members are considered to be the receiver side of the channel.
pub trait ChannelSpec {
    /// Channel that can be used up to once over its lifetime.
    type Oneshot<T>;
    /// Channel that is a non-blocking queue that can be used multiple times per message.
    type Queue<T>;

    /// Creates a new oneshot channel, the sender half of which is boxed.
    fn new_oneshot<T: 'static>(&self) -> (Box<dyn Sender<Value = T>>, Self::Oneshot<T>);

    /// Creates a new queue channel, the sender half of which is boxed.
    fn new_queue<T: 'static>(&self) -> (Box<dyn Sender<Value = T>>, Self::Queue<T>);
}

/// [`ChannelSpec`] for thread-safe synchronous channels.
pub struct SyncChannels;
#[cfg(feature = "tokio")]
/// [`ChannelSpec`] for Tokio channels.
pub struct TokioChannels;

impl ChannelSpec for SyncChannels {
    type Oneshot<T> = (self::oneshot::Receiver<T>, self::parker::Parker);

    type Queue<T> = std::sync::mpsc::Receiver<T>;

    fn new_oneshot<T: 'static>(&self) -> (Box<dyn Sender<Value = T>>, Self::Oneshot<T>) {
        let (send, recv) = self::oneshot::channel();
        let (unparker, parker) = self::parker::new(Some(send));
        (Box::new(unparker), (recv, parker))
    }

    fn new_queue<T: 'static>(&self) -> (Box<dyn Sender<Value = T>>, Self::Queue<T>) {
        let (send, recv) = std::sync::mpsc::channel();
        (Box::new(send), recv)
    }
}

#[cfg(feature = "tokio")]
impl ChannelSpec for TokioChannels {
    type Oneshot<T> = tokio::sync::oneshot::Receiver<T>;

    type Queue<T> = tokio::sync::mpsc::UnboundedReceiver<T>;

    fn new_oneshot<T: 'static>(&self) -> (Box<dyn Sender<Value = T>>, Self::Oneshot<T>) {
        let (send, recv) = tokio::sync::oneshot::channel();
        (Box::new(Some(send)), recv)
    }

    fn new_queue<T: 'static>(&self) -> (Box<dyn Sender<Value = T>>, Self::Queue<T>) {
        let (send, recv) = tokio::sync::mpsc::unbounded_channel();
        (Box::new(send), recv)
    }
}

pub mod parker {
    //! Utilities for temporarily parking threads, awaiting some activity on another thread.
    //!
    //! These can be used to turn non-blocking channels into blocking ones in synchronous code.
    //! They are designed for mpsc usecases, allowing multiple threads to unpark one thread.

    use std::mem::ManuallyDrop;
    use std::sync::{
        atomic::{AtomicPtr, Ordering},
        Arc, Weak,
    };
    use std::thread::Thread;

    /// Global location whose address we can use to indicate that a [`Parker`] should skip parking.
    static mut SKIP_PARKING: std::mem::MaybeUninit<Thread> = std::mem::MaybeUninit::uninit();

    /// A wrapped [`Sender`][super::Sender] that can unpark a thread blocked by a [`Parker`].
    #[derive(Clone, Debug, Default)]
    pub struct Unparker<S>(S, ManuallyDrop<Arc<AtomicPtr<Thread>>>);
    /// A synchronization primitive for parking the thread indefinitely pending activity
    /// on a thread with an [`Unparker`].
    #[derive(Debug)]
    pub struct Parker(Weak<AtomicPtr<Thread>>);

    /// Creates a new [`Unparker`] from the provided sender,
    /// also returning a [`Parker`] for that unparker.
    pub fn new<S>(sender: S) -> (Unparker<S>, Parker) {
        let arc = Arc::new(AtomicPtr::new(std::ptr::null_mut()));
        let weak = Arc::downgrade(&arc);
        (Unparker(sender, ManuallyDrop::new(arc)), Parker(weak))
    }

    impl<S> Drop for Unparker<S> {
        fn drop(&mut self) {
            let Some(arc) = Arc::into_inner(unsafe { ManuallyDrop::take(&mut self.1) }) else {
                return;
            };
            let ptr = arc.into_inner();
            if ptr != unsafe { SKIP_PARKING.as_mut_ptr() } {
                if let Some(th) = unsafe { ptr.as_ref() } {
                    th.clone().unpark();
                }
            }
        }
    }

    impl<S> Unparker<S> {
        /// Unparks a thread that is blocked by a [`Parker`].
        /// If no thread is parked for this unparker, skips the next parking operation.
        ///
        /// This generally doesn't need to be called manually unless `S` is not a sender.
        pub fn unpark(&self) {
            let ptr = self.1.swap(unsafe { SKIP_PARKING.as_mut_ptr() }, Ordering::AcqRel);
            if ptr != unsafe { SKIP_PARKING.as_mut_ptr() } {
                if let Some(th) = unsafe { ptr.as_ref() } {
                    th.clone().unpark();
                }
            }
        }
    }

    impl<S: super::Sender> super::Sender for Unparker<S> {
        type Value = <S as super::Sender>::Value;

        fn send(&mut self, value: Self::Value) -> super::SendCont {
            let result = self.0.send(value);
            if result != super::SendCont::Closed {
                self.unpark();
            }
            result
        }

        fn may_send(&self) -> bool {
            self.0.may_send()
        }
    }

    impl Parker {
        /// Block this thread until either all [`Unparker`]s are dropped or
        /// until one of them unparks this thread.
        pub fn park(&self) {
            let Some(strong) = self.0.upgrade() else {
                return;
            };
            let mut th = std::thread::current();
            if strong
                .compare_exchange(
                    std::ptr::null_mut(),
                    &mut th as *mut Thread,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
            {
                if Arc::into_inner(strong).is_none() {
                    std::thread::park();
                } else {
                    // No unparkers left!
                    return;
                }
            }
            let Some(strong) = self.0.upgrade() else {
                return;
            };
            strong.store(std::ptr::null_mut(), Ordering::Release);
        }
    }
}

pub mod oneshot {
    //! An implementation of a non-blocking oneshot channel.
    //!
    //! This is essentially just a [`OnceLock`][std::sync::OnceLock] in an [`Arc`].
    //! It offers no means of blocking.
    //! This makes it safe to use in `async` contexts, but means that users of these types
    //! need to work out synchronization to prevent premature reads.
    //!
    //! Consider using a [`Parker`][super::parker::Parker] if synchronization is needed.

    use super::parker::Parker;
    use std::sync::{Arc, OnceLock, Weak};

    /// The sender portion of a oneshot channel.
    #[derive(Debug)]
    pub struct Sender<T>(Weak<OnceLock<T>>);

    /// The reciever portion of a oneshot channel.
    #[derive(Debug)]
    pub struct Receiver<T>(Arc<OnceLock<T>>);

    /// Creates a new oneshot channel for sending single values.
    pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
        let strong = Arc::new(OnceLock::new());
        let weak = Arc::downgrade(&strong);
        (Sender(weak), Receiver(strong))
    }

    impl<T> Sender<T> {
        /// Returns `true` if sends on this channel are guaranteed to fail.
        pub fn is_closed(&self) -> bool {
            self.0.strong_count() == 0
        }
        /// Attempts to send a value over this channel.
        pub fn send(self, value: T) -> Result<(), T> {
            let Some(strong) = self.0.upgrade() else {
                return Err(value);
            };
            strong.set(value)?;
            if let Some(existing) = Arc::into_inner(strong).and_then(OnceLock::into_inner) {
                Err(existing)
            } else {
                Ok(())
            }
        }
    }

    impl<T> super::Sender for Option<Sender<T>> {
        type Value = T;

        fn send(&mut self, value: Self::Value) -> super::SendCont {
            let Some(sender) = self.take() else {
                return super::SendCont::Closed;
            };
            if sender.send(value).is_ok() {
                super::SendCont::SentClosed
            } else {
                super::SendCont::Closed
            }
        }

        fn may_send(&self) -> bool {
            self.as_ref().is_some_and(|snd| !snd.is_closed())
        }
    }

    impl<T> Receiver<T> {
        /// Returns a reference to the value that is ready to be received, if any.
        pub fn peek(&self) -> Option<&T> {
            self.0.get()
        }
        /// Receives a value over this channel, consuming the receiver.
        ///
        /// This method never blocks.
        pub fn recv_nonblocking(self) -> Option<T> {
            Arc::into_inner(self.0).and_then(OnceLock::into_inner)
        }
        /// Receives a value over this channel, blocking until one is present.
        pub fn recv(self, parker: &Parker) -> Option<T> {
            if self.peek().is_none() {
                parker.park();
            }
            self.recv_nonblocking()
        }
    }
}

#[cfg(test)]
mod tests {
    /// Park, expecting the thread to be blocked for 100 to 2000 ms.
    /// Panic if the time falls outside of this range.
    fn timed_park() {
        let then = std::time::Instant::now();
        std::thread::park_timeout(std::time::Duration::from_secs(2));
        let now = std::time::Instant::now();
        let diff = now - then;
        if diff < std::time::Duration::from_millis(100) {
            panic!("probable non-block; parked for {}ms", diff.as_millis());
        }
        if diff >= std::time::Duration::from_secs(2) {
            panic!("probable deadlock; parked for {}ms", diff.as_millis());
        }
    }
    #[test]
    fn parker_slow_unpark() {
        let (unparker, parker) = super::parker::new(());
        let current = std::thread::current();
        std::thread::spawn(move || {
            parker.park();
            current.unpark();
        });
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(200));
            unparker.unpark();
        });
        timed_park();
    }
    #[test]
    fn parker_slow_park() {
        let (unparker, parker) = super::parker::new(());
        let current = std::thread::current();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(200));
            parker.park();
            current.unpark();
        });
        std::thread::spawn(move || {
            unparker.unpark();
        });
        timed_park();
    }
    #[test]
    fn unparker_drop_slow_unpark() {
        let (unparker1, parker) = super::parker::new(());
        let unparker2 = unparker1.clone();
        let current = std::thread::current();
        std::thread::spawn(move || {
            parker.park();
            current.unpark();
        });
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(200));
            std::mem::drop(unparker1);
            std::mem::drop(unparker2);
        });
        timed_park();
    }
    #[test]
    fn unparker_drop_slow_park() {
        let (unparker1, parker) = super::parker::new(());
        let unparker2 = unparker1.clone();
        let current = std::thread::current();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(200));
            parker.park();
            current.unpark();
        });
        std::thread::spawn(move || {
            std::mem::drop(unparker1);
            std::mem::drop(unparker2);
        });
        timed_park();
    }
}
