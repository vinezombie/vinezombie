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
