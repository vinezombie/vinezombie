use std::{alloc::Layout, ptr::NonNull};

/// Variably-sized types.
///
/// # Safety
/// This trait is entirely logic for building pointers with detached metadata,
/// reimplementing what rustc currently only exposes in nightly.
/// Undefined behavior is very easy to run into by accident.
pub unsafe trait DynSized {
    type Metadata: Sized;
    const ALIGN: usize;
    type Ref<'a>: Sized
    where
        Self: 'a;
    type RefMut<'a>: Sized
    where
        Self: 'a;
    unsafe fn layout(meta: &Self::Metadata) -> Layout;
    unsafe fn make_ref(ptr: NonNull<u8>, meta: &Self::Metadata) -> Self::Ref<'_>;
    unsafe fn make_ref_mut(ptr: NonNull<u8>, meta: &Self::Metadata) -> Self::RefMut<'_>;
    unsafe fn drop_in_place(ptr: NonNull<u8>, meta: &Self::Metadata);
}

unsafe impl<T: Sized> DynSized for T {
    type Metadata = ();
    const ALIGN: usize = std::mem::align_of::<T>();
    type Ref<'a>
        = &'a T
    where
        Self: 'a;
    type RefMut<'a>
        = &'a mut T
    where
        Self: 'a;
    unsafe fn layout(_: &Self::Metadata) -> Layout {
        std::alloc::Layout::new::<T>()
    }
    unsafe fn make_ref(ptr: NonNull<u8>, _: &Self::Metadata) -> Self::Ref<'_> {
        ptr.cast::<T>().as_ref()
    }
    unsafe fn make_ref_mut(ptr: NonNull<u8>, _: &Self::Metadata) -> Self::RefMut<'_> {
        ptr.cast::<T>().as_mut()
    }
    unsafe fn drop_in_place(ptr: NonNull<u8>, _: &Self::Metadata) {
        std::ptr::drop_in_place(ptr.cast::<T>().as_mut());
    }
}

unsafe impl<T: Sized> DynSized for [T] {
    type Metadata = usize;
    const ALIGN: usize = std::mem::align_of::<T>();
    type Ref<'a>
        = &'a [T]
    where
        Self: 'a;
    type RefMut<'a>
        = &'a mut [T]
    where
        Self: 'a;
    unsafe fn layout(meta: &Self::Metadata) -> Layout {
        std::alloc::Layout::array::<T>(*meta).unwrap()
    }
    unsafe fn make_ref(ptr: NonNull<u8>, count: &Self::Metadata) -> Self::Ref<'_> {
        std::slice::from_raw_parts(ptr.cast::<T>().as_ptr(), *count)
    }
    unsafe fn make_ref_mut(ptr: NonNull<u8>, count: &Self::Metadata) -> Self::RefMut<'_> {
        std::slice::from_raw_parts_mut(ptr.cast::<T>().as_ptr(), *count)
    }
    unsafe fn drop_in_place(ptr: NonNull<u8>, count: &Self::Metadata) {
        let mut ptr = ptr.cast::<T>().as_ptr();
        for _ in 0..*count {
            std::ptr::drop_in_place(ptr);
            ptr = ptr.add(1);
        }
    }
}
