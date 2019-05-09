#![no_std]

#![feature(allocator_api)]
#![feature(const_fn)]
#![feature(ptr_internals)]
#![feature(try_reserve)]
#![feature(dropck_eyepatch)]

extern crate alloc;
extern crate core;
use core::alloc::Alloc;
use core::ptr::{NonNull, Unique};
use core::mem;
use alloc::alloc::{Layout, Global};
use alloc::alloc::handle_alloc_error;
use alloc::boxed::Box;
use alloc::collections::CollectionAllocErr::{self, *};

fn capacity_overflow() -> ! {
    panic!("capacity overflow")
}

#[inline]
fn alloc_guard(alloc_size: usize) -> Result<(), CollectionAllocErr> {
    if mem::size_of::<usize>() < 8 && alloc_size > core::isize::MAX as usize {
        Err(CapacityOverflow)
    } else {
        Ok(())
    }
}

pub struct PureHeap<T, A: Alloc = Global> {
    pub ptr: Unique<T>,
    a: A,
}

impl<T, A: Alloc> PureHeap<T, A> {
    /// Gets a raw pointer to the start of the allocation. Note that this is
    /// Unique::empty() if `cap = 0` or T is zero-sized. In the former case, you must
    /// be careful.
    pub fn ptr(&self) -> *mut T {
        self.ptr.as_ptr()
    }

    /// Returns a shared reference to the allocator backing this RawVec.
    pub fn alloc(&self) -> &A {
        &self.a
    }

    /// Returns a mutable reference to the allocator backing this RawVec.
    pub fn alloc_mut(&mut self) -> &mut A {
        &mut self.a
    }

    fn current_layout(&self) -> Option<Layout> {
            unsafe {
                let align = mem::align_of::<T>();
                let size = mem::size_of::<T>();
                Some(Layout::from_size_align_unchecked(size, align))
            }
    }
}

impl<T, A: Alloc> PureHeap<T, A> {
    pub unsafe fn dealloc_buffer(&mut self) {
        let elem_size = mem::size_of::<T>();
        if elem_size != 0 {
            if let Some(layout) = self.current_layout() {
                self.a.dealloc(NonNull::from(self.ptr).cast(), layout);
            }
        }
    }
}

unsafe impl<#[may_dangle] T, A: Alloc> Drop for PureHeap<T, A> {
    fn drop(&mut self) {
        unsafe { self.dealloc_buffer(); }
    }
}

impl<T, A: Alloc> PureHeap<T, A> {

    pub(crate) fn new_in(a: A) -> Self {
        PureHeap::allocate_in(true, a)
    }

    fn allocate_in(zeroed: bool, mut a: A) -> Self {
        let elem_size = mem::size_of::<T>();

        alloc_guard(elem_size).unwrap_or_else(|_| capacity_overflow());

        // handles ZSTs and `cap = 0` alike
        let ptr = if elem_size == 0 {
            NonNull::<T>::dangling()
        } else {
            let align = mem::align_of::<T>();
            let layout = Layout::from_size_align(elem_size, align).unwrap();
            let result = if zeroed {
                unsafe { a.alloc_zeroed(layout) }
            } else {
                unsafe { a.alloc(layout) }
            };
            match result {
                Ok(ptr) => ptr.cast(),
                Err(_) => handle_alloc_error(layout),
            }
        };

        PureHeap {
            ptr: ptr.into(),
            a,
        }
    }
}

impl<T> PureHeap<T, Global> {
    pub fn new() -> Self {
        Self::new_in(Global)
    }
}

impl<T, A: Alloc> PureHeap<T, A> {
    pub fn new_with_allocator(a: A) -> Self {
        Self::new_in(a)
    }
}

impl<T, A: Alloc> PureHeap<T, A> {
    pub unsafe fn from_raw_parts(ptr: *mut T, a: A) -> Self {
        PureHeap {
            ptr: Unique::new_unchecked(ptr),
            a,
        }
    }
}

impl<T> PureHeap<T, Global> {
    pub fn from_box(mut slice: Box<[T]>) -> Self {
        unsafe {
            let result = PureHeap::from_raw_parts(slice.as_mut_ptr(), Global);
            mem::forget(slice);
            result
        }
    }
}

#[cfg(test)]
mod test {
    use crate::*;

    #[test]
    fn test_naive_i32() {
        let t = PureHeap::<i32>::new();
        assert_ne!(t.ptr.as_ptr(), core::ptr::null_mut());
    }

    extern crate std;
    use std::alloc::System;

    #[test]
    fn test_alloc_with_system() {
        let t = PureHeap::<i32, System>::new_with_allocator(System);
        assert_ne!(t.ptr.as_ptr(), core::ptr::null_mut());
    }

    //#[test]
    //#[should_panic] // This should OOM and cargo test cannot unwind it!
    //fn test_big() {
    //    use std::boxed::Box;
    //    let _ = Box::new([[0;1000];1000]);
    //}

    #[test]
    fn test_pure_big() {
        let t = PureHeap::<[[i32;100];1000]>::new();
        assert_ne!(t.ptr.as_ptr(), core::ptr::null_mut());
    }

    struct DummyStruct;
    #[test]
    fn test_zero() {
        use core::ptr::NonNull;
        let t = PureHeap::<DummyStruct>::new();
        assert_eq!(t.ptr.as_ptr(), NonNull::<_>::dangling().as_ptr());
    }

    #[test]
    fn test_drop() {
        let t = PureHeap::<[[i32;10000];1000]>::new();
        let ptr = t.ptr.as_ptr();
        mem::drop(t);
        let t = PureHeap::<[[i32;10000];1000]>::new();
        assert_eq!(ptr, t.ptr.as_ptr());
    }
}
