#![allow(unknown_lints)]
//! An implementation of a non-blocking oneshot channel.
//!
//! This is essentially just a [`OnceLock`][std::sync::OnceLock] under
//! thread-safe shared ownership. It may briefly block to avoid data races,
//! but should be unable to deadlock.
//!
//! Consider using a [`Parker`][super::parker::Parker] if synchronization is needed.

// Unknown lints lint is disabled because dropping_references was added after
// 1.70 (our MSRV). We drop references to be absolutely sure they won't live
// at the same time as the memory they point to getting dropped.

use super::parker::Parker;
use std::{
    cell::UnsafeCell,
    mem::MaybeUninit,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Once,
    },
};

// Summary: This is an mpsc oneshot channel. Only one value can be sent over the channel,
// but it may come from one of several senders.
// The receiver owns an in-transit value and is responsible for cleaning it up.
// The senders have no access to the sent value.

struct Inner<T> {
    sender_count: AtomicUsize,
    recver: AtomicBool,
    lock: Once,
    // Manually implement OnceLock so that we can deinit the value but keep the lock.
    value: UnsafeCell<MaybeUninit<T>>,
}

/// The receiver portion of a oneshot channel.
#[derive(Debug)]
pub struct Receiver<T>(std::ptr::NonNull<Inner<T>>);

/// The sender portion of a oneshot channel.
#[derive(Debug)]
pub struct Sender<T>(std::ptr::NonNull<Inner<T>>);

unsafe impl<T: Send> Send for Receiver<T> {}
unsafe impl<T: Send> Send for Sender<T> {}
// You can use a `Receiver` to get a `&T`, but only with one method that requires `T: Sync`.
unsafe impl<T> Sync for Receiver<T> {}
// You cannot use a `Sender` to get a `&T`.
unsafe impl<T> Sync for Sender<T> {}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        unsafe {
            self.0.as_ref().sender_count.fetch_add(1, Ordering::Relaxed);
        }
        Self(self.0)
    }
}

// TODO: All the Acquire orderings going forward are chosen defensively.
// Evaluate if they can be relaxed.

impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        let inner_ref = unsafe { self.0.as_ref() };
        inner_ref.recver.store(false, Ordering::Release);
        // Destroy the value, if present.
        // Do NOT use Once::is_completed
        let mut has_value = true;
        inner_ref.lock.call_once(|| has_value = false);
        if has_value {
            unsafe { inner_ref.value.get().as_mut().unwrap_unchecked().assume_init_drop() };
        }
        if inner_ref.sender_count.load(Ordering::Acquire) == 0 {
            #[allow(dropping_references)]
            std::mem::drop(inner_ref);
            std::mem::drop(unsafe { Box::from_raw(self.0.as_ptr()) });
        }
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        let inner_ref = unsafe { self.0.as_ref() };
        if inner_ref.sender_count.fetch_sub(1, Ordering::Release) == 1 {
            #[allow(clippy::collapsible_if)]
            if !inner_ref.recver.load(Ordering::Acquire) {
                #[allow(dropping_references)]
                std::mem::drop(inner_ref);
                std::mem::drop(unsafe { Box::from_raw(self.0.as_ptr()) });
            }
        }
    }
}

/// Creates a new oneshot channel for sending single values.
pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
    let inner = Box::new(Inner {
        sender_count: AtomicUsize::new(1),
        recver: AtomicBool::new(true),
        lock: Once::new(),
        value: UnsafeCell::new(MaybeUninit::uninit()),
    });
    let inner = std::ptr::NonNull::new(Box::into_raw(inner)).unwrap();
    (Sender(inner), Receiver(inner))
}

impl<T> Receiver<T> {
    /// Returns `false` if a value has been sent over this channel.
    pub fn is_empty(&self) -> bool {
        let inner_ref = unsafe { self.0.as_ref() };
        !inner_ref.lock.is_completed()
    }
    /// Returns a reference to the value that is ready to be received, if any.
    pub fn peek(&self) -> Option<&T>
    where
        T: Sync,
    {
        let inner_ref = unsafe { self.0.as_ref() };
        inner_ref
            .lock
            .is_completed()
            .then(|| unsafe { inner_ref.value.get().as_ref().unwrap_unchecked().assume_init_ref() })
    }
    /// Receives a value over this channel, blocking only to allow a send-in-progress
    /// to complete.
    pub fn recv_now(self) -> Option<T> {
        let ptr = self.0;
        let inner_ref = unsafe { ptr.as_ref() };
        // Absolutely do not want the destructor to run here.
        // We have the references we need to the data anyway.
        std::mem::forget(self);
        inner_ref.recver.store(false, Ordering::Release);
        let mut has_value = true;
        inner_ref.lock.call_once(|| has_value = false);
        let retval = has_value.then(|| unsafe {
            inner_ref.value.get().as_ref().unwrap_unchecked().assume_init_read()
        });
        if inner_ref.sender_count.load(Ordering::Acquire) == 0 {
            #[allow(dropping_references)]
            std::mem::drop(inner_ref);
            std::mem::drop(unsafe { Box::from_raw(ptr.as_ptr()) });
        }
        retval
    }
    /// Receives a value over this channel. Parks using the provided `parker` if
    /// no value is immediately available to be received.
    pub fn recv(self, parker: &Parker) -> Option<T> {
        if self.is_empty() {
            parker.park();
        }
        self.recv_now()
    }
}

impl<T> Sender<T> {
    /// Returns `true` if sends on this channel are guaranteed to fail.
    pub fn is_closed(&self) -> bool {
        let inner_ref = unsafe { self.0.as_ref() };
        !inner_ref.recver.load(Ordering::Relaxed) || inner_ref.lock.is_completed()
    }
    /// Attempts to send a value over this channel.
    pub fn send(self, value: T) -> Result<(), T> {
        let mut value = Some(value);
        let inner_ref = unsafe { self.0.as_ref() };
        if inner_ref.recver.load(Ordering::Relaxed) {
            inner_ref.lock.call_once(|| unsafe {
                let value = value.take().unwrap_unchecked();
                inner_ref.value.get().as_mut().unwrap_unchecked().write(value);
            });
        }
        match value {
            Some(value) => Err(value),
            None => Ok(()),
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
