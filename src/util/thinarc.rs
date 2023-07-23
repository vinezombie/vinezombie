use super::DynSized;
use std::{
    alloc::Layout,
    ptr::NonNull,
    sync::atomic::{AtomicUsize, Ordering},
};

#[repr(C)]
struct ThinArcInner<T: DynSized + ?Sized> {
    rc: AtomicUsize,
    meta: T::Metadata,
    value: T,
}

/// Atomic reference-counted box that has no notion of weak references.
#[repr(transparent)]
pub struct ThinArc<T: DynSized + ?Sized>(NonNull<u8>, std::marker::PhantomData<ThinArcInner<T>>);

impl<T> ThinArc<T> {
    pub fn new(value: T) -> Self {
        let inner = ThinArcInner { rc: AtomicUsize::new(1), meta: (), value };
        let layout = Layout::for_value(&inner);
        unsafe {
            // Safety: SArcInner has an atomic usize in it, thus is never zero-sized.
            let ptr = std::alloc::alloc(layout) as *mut ThinArcInner<T>;
            let Some(nonnull) = NonNull::new(ptr) else {
                std::alloc::handle_alloc_error(layout);
            };
            nonnull.as_ptr().write(inner);
            Self(nonnull.cast(), std::marker::PhantomData)
        }
    }
    pub fn try_unwrap(mut self) -> Result<T, Self> {
        unsafe {
            if self.ret_rc().load(Ordering::Relaxed) == 1 {
                let retval = Ok(self.offsets().1.cast::<T>().read());
                self.dealloc();
                std::mem::forget(self);
                retval
            } else {
                Err(self)
            }
        }
    }
}

impl<T: DynSized + ?Sized> ThinArc<T> {
    /// Takes a pointer to the metadata and calculates the offset to the start of the value.
    #[inline(always)]
    unsafe fn offsets(&self) -> (*mut u8, *mut u8) {
        let ptr = self.0.as_ptr();
        let (layout, offset_meta) =
            Layout::new::<AtomicUsize>().extend(Layout::new::<T::Metadata>()).unwrap();
        let ptr_meta = ptr.add(offset_meta);
        let offset_value = layout.extend(Layout::from_size_align_unchecked(0, T::ALIGN)).unwrap().1;
        let ptr_value = ptr.add(offset_value);
        (ptr_meta, ptr_value)
    }
    fn ret_rc(&self) -> &AtomicUsize {
        // Safety: ThinArcInner is repr(C),
        // and the refcount is at the start of the struct.
        unsafe { self.0.cast::<AtomicUsize>().as_ref() }
    }
    fn ref_meta(&self) -> &T::Metadata {
        unsafe { self.offsets().0.cast::<T::Metadata>().as_ref().unwrap_unchecked() }
    }
    fn ref_value(&self) -> T::Ref<'_> {
        unsafe {
            let (ptr_meta, ptr_value) = self.offsets();
            T::make_ref(
                NonNull::new_unchecked(ptr_value),
                ptr_meta.cast::<T::Metadata>().as_ref().unwrap_unchecked(),
            )
        }
    }
    unsafe fn dealloc(&mut self) {
        let mut layout = Layout::new::<AtomicUsize>();
        layout = layout.extend(Layout::new::<T::Metadata>()).unwrap().0;
        layout = layout.extend(T::layout(self.ref_meta())).unwrap().0;
        std::alloc::dealloc(self.0.as_ptr(), layout);
    }
}

impl<T: DynSized + ?Sized> Clone for ThinArc<T> {
    fn clone(&self) -> Self {
        self.ret_rc().fetch_add(1, Ordering::Relaxed);
        ThinArc(self.0, std::marker::PhantomData)
    }
}

unsafe impl<T: DynSized + Send + Sync + ?Sized> Send for ThinArc<T> {}
unsafe impl<T: DynSized + Send + Sync + ?Sized> Sync for ThinArc<T> {}
impl<T: DynSized + ?Sized> Unpin for ThinArc<T> {}

// TODO: All the other impls.

impl<T> std::ops::Deref for ThinArc<T>
where
    for<'a> T: DynSized<Ref<'a> = &'a T> + 'a,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.ref_value()
    }
}

impl<T: DynSized + ?Sized> Drop for ThinArc<T> {
    fn drop(&mut self) {
        unsafe {
            if self.ret_rc().fetch_sub(1, Ordering::Release) == 1 {
                let (ptr_meta, ptr_value) = self.offsets();
                T::drop_in_place(
                    NonNull::new_unchecked(ptr_value),
                    ptr_meta.cast::<T::Metadata>().as_ref().unwrap_unchecked(),
                );
                self.dealloc();
            }
        }
    }
}
