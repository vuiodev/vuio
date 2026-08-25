use std::alloc::{Layout, alloc_zeroed, dealloc};
use std::ffi::CStr;
use std::ptr::NonNull;

use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionInfo {
    pub name: String,
    pub version: String,
}

pub fn c_string(ptr: *mut i8) -> String {
    if ptr.is_null() {
        return String::new();
    }

    unsafe { CStr::from_ptr(ptr as *const std::ffi::c_char) }
        .to_string_lossy()
        .into_owned()
}

#[derive(Debug)]
pub struct AlignedBuffer {
    base: NonNull<u8>,
    aligned: NonNull<u8>,
    layout: Layout,
}

impl AlignedBuffer {
    pub fn new(size: usize, alignment: usize) -> Result<Self> {
        let alignment = alignment.max(1).next_power_of_two();
        let header_size = 2 * std::mem::size_of::<usize>();
        let total_size = size
            .checked_add(alignment)
            .and_then(|value| value.checked_add(header_size))
            .ok_or(Error::AllocationFailed { size, alignment })?;
        let layout = Layout::from_size_align(total_size, std::mem::align_of::<usize>())
            .map_err(|_| Error::AllocationFailed { size, alignment })?;
        let base = unsafe { alloc_zeroed(layout) };
        let base = NonNull::new(base).ok_or(Error::AllocationFailed { size, alignment })?;
        let aligned_addr =
            (base.as_ptr() as usize + header_size + alignment - 1) & !(alignment - 1);
        let aligned = NonNull::new(aligned_addr as *mut u8)
            .ok_or(Error::AllocationFailed { size, alignment })?;

        Ok(Self {
            base,
            aligned,
            layout,
        })
    }

    pub fn as_ptr(&self) -> *mut u8 {
        self.aligned.as_ptr()
    }
}

impl Drop for AlignedBuffer {
    fn drop(&mut self) {
        unsafe {
            dealloc(self.base.as_ptr(), self.layout);
        }
    }
}

unsafe extern "C" fn xaac_alloc(size: u32, alignment: u32) -> *mut std::ffi::c_void {
    let requested_align = alignment
        .max(libxaac_sys::DEFAULT_MEM_ALIGN_8)
        .next_power_of_two() as usize;
    let header_words = 2 * std::mem::size_of::<usize>();
    let total_size = size as usize + requested_align + header_words;
    let layout =
        Layout::from_size_align(total_size, std::mem::align_of::<usize>()).expect("valid layout");
    let base = unsafe { alloc_zeroed(layout) };
    if base.is_null() {
        return std::ptr::null_mut();
    }

    let aligned_addr =
        (base as usize + header_words + requested_align - 1) & !(requested_align - 1);
    let aligned = aligned_addr as *mut u8;
    let meta = unsafe { aligned.cast::<usize>().sub(2) };
    unsafe {
        meta.write(base as usize);
        meta.add(1).write(total_size);
    }
    aligned.cast()
}

unsafe extern "C" fn xaac_free(ptr: *mut std::ffi::c_void) {
    if ptr.is_null() {
        return;
    }

    let meta = unsafe { (ptr as *mut usize).sub(2) };
    let base = unsafe { meta.read() } as *mut u8;
    let total_size = unsafe { meta.add(1).read() };
    let layout =
        Layout::from_size_align(total_size, std::mem::align_of::<usize>()).expect("valid layout");
    unsafe {
        dealloc(base, layout);
    }
}

pub(crate) fn encoder_alloc() -> Option<unsafe extern "C" fn(u32, u32) -> *mut std::ffi::c_void> {
    Some(xaac_alloc)
}

pub(crate) fn encoder_free() -> Option<unsafe extern "C" fn(*mut std::ffi::c_void)> {
    Some(xaac_free)
}
