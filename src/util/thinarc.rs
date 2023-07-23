use std::{
    ptr::NonNull,
    sync::atomic::{AtomicUsize, Ordering}, alloc::Layout,
};

#[repr(C)]
struct ThinArcInner<T: ?Sized>(AtomicUsize, T);

/// Atomic reference-counted box that has no notion of weak references.
#[repr(transparent)]
pub struct ThinArc<T: ?Sized>(NonNull<ThinArcInner<T>>);

impl<T> ThinArc<T> {
    pub fn new(value: T) -> Self {
        let inner = ThinArcInner(AtomicUsize::new(1), value);
        let layout = Layout::for_value(&inner);
        unsafe {
            // Safety: SArcInner has an atomic usize in it, thus is never zero-sized.
            let ptr = std::alloc::alloc(layout) as *mut ThinArcInner<T>;
            let Some(nonnull) = NonNull::new(ptr) else {
                std::alloc::handle_alloc_error(layout);
            };
            nonnull.as_ptr().write(inner);
            Self(nonnull)
        }
    }
    pub fn try_unwrap(mut self) -> Option<T> {
        unsafe {
            if self.0.as_ref().0.fetch_sub(1, Ordering::Release) == 1 {
                let retval = Some((&self.0.as_ref().1 as *const T).read());
                self.dealloc();
                std::mem::forget(self);
                retval
            } else {
                // We just de-incremented the refcount. Don't do it again.
                std::mem::forget(self);
                None
            }
        }
    }
}

impl<T: ?Sized> ThinArc<T> {
    unsafe fn dealloc(&mut self) {
        let layout = Layout::for_value(self.0.as_ref());
        let raw = self.0.as_ptr() as *mut u8;
        std::alloc::dealloc(raw, layout);
    }
}

impl<T: ?Sized> Clone for ThinArc<T> {
    fn clone(&self) -> Self {
        unsafe {
            self.0.as_ref().0.fetch_add(1, Ordering::Relaxed);
        }
        ThinArc(self.0)
    }
}

// TODO: All the other impls.

impl<T: ?Sized> std::ops::Deref for ThinArc<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &self.0.as_ref().1 }
    }
}

impl<T: ?Sized> Drop for ThinArc<T> {
    fn drop(&mut self) {
        unsafe {
            if self.0.as_ref().0.fetch_sub(1, Ordering::Release) == 1 {
                (&mut self.0.as_mut().1 as *mut T).drop_in_place();
                self.dealloc();
            }
        }
    }
}
