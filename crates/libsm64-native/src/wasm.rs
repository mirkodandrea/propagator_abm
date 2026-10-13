//! `malloc` and friends for the C side on wasm32-unknown-unknown, which has no
//! libc. Each block is preceded by a 16-byte header holding its size.
use std::alloc::{alloc, alloc_zeroed, dealloc, realloc as rust_realloc, Layout};

const HEADER: usize = 16;

fn layout(size: usize) -> Layout {
    Layout::from_size_align(size + HEADER, HEADER).unwrap()
}

#[no_mangle]
pub unsafe extern "C" fn malloc(size: usize) -> *mut u8 {
    let p = alloc(layout(size));
    if p.is_null() { return p; }
    *(p as *mut usize) = size;
    p.add(HEADER)
}

#[no_mangle]
pub unsafe extern "C" fn calloc(n: usize, size: usize) -> *mut u8 {
    let Some(size) = n.checked_mul(size) else { return std::ptr::null_mut() };
    let p = alloc_zeroed(layout(size));
    if p.is_null() { return p; }
    *(p as *mut usize) = size;
    p.add(HEADER)
}

#[no_mangle]
pub unsafe extern "C" fn free(ptr: *mut u8) {
    if ptr.is_null() { return; }
    let base = ptr.sub(HEADER);
    dealloc(base, layout(*(base as *const usize)));
}

#[no_mangle]
pub unsafe extern "C" fn realloc(ptr: *mut u8, size: usize) -> *mut u8 {
    if ptr.is_null() { return malloc(size); }
    let base = ptr.sub(HEADER);
    let old = *(base as *const usize);
    let p = rust_realloc(base, layout(old), size + HEADER);
    if p.is_null() { return p; }
    *(p as *mut usize) = size;
    p.add(HEADER)
}
