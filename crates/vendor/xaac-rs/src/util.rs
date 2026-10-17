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

/// Blocks at least this large are mapped from the system directly rather than
/// taken from the heap.
///
/// The encoder asks for two blocks, of 12 MB and 44 MB, sized for a USAC
/// encoder whatever the profile; AAC-LC writes well under a megabyte of them.
/// libxaac-sys builds the encoder to trust that its memory arrives zeroed, and
/// untouched zero pages cost nothing — but only while they are fresh. The heap
/// keeps freed blocks this size for reuse, and to hand one back zeroed it has to
/// clear it, which writes every page: the first encoder cost under a megabyte
/// and every one after it the full 55 MB again. Mapping the blocks directly
/// keeps each encoder's untouched pages unallocated and returns the rest to the
/// system when it is dropped.
#[cfg(unix)]
const MAP_THRESHOLD: usize = 1 << 20;

/// Zeroed memory for the encoder. See `MAP_THRESHOLD`.
unsafe fn zeroed_block(total_size: usize) -> *mut u8 {
    #[cfg(unix)]
    if total_size >= MAP_THRESHOLD {
        let mapped = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                total_size,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANON,
                -1,
                0,
            )
        };
        return if mapped == libc::MAP_FAILED {
            std::ptr::null_mut()
        } else {
            mapped.cast()
        };
    }
    let layout =
        Layout::from_size_align(total_size, std::mem::align_of::<usize>()).expect("valid layout");
    unsafe { alloc_zeroed(layout) }
}

/// Release a block from [`zeroed_block`] of the same `total_size`.
unsafe fn release_block(base: *mut u8, total_size: usize) {
    #[cfg(unix)]
    if total_size >= MAP_THRESHOLD {
        unsafe { libc::munmap(base.cast(), total_size) };
        return;
    }
    let layout =
        Layout::from_size_align(total_size, std::mem::align_of::<usize>()).expect("valid layout");
    unsafe { dealloc(base, layout) };
}

unsafe extern "C" fn xaac_alloc(size: u32, alignment: u32) -> *mut std::ffi::c_void {
    let requested_align = alignment
        .max(libxaac_sys::DEFAULT_MEM_ALIGN_8)
        .next_power_of_two() as usize;
    let header_words = 2 * std::mem::size_of::<usize>();
    let total_size = size as usize + requested_align + header_words;
    let base = unsafe { zeroed_block(total_size) };
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
    unsafe { release_block(base, total_size) };
}

pub(crate) fn encoder_alloc() -> Option<unsafe extern "C" fn(u32, u32) -> *mut std::ffi::c_void> {
    Some(xaac_alloc)
}

pub(crate) fn encoder_free() -> Option<unsafe extern "C" fn(*mut std::ffi::c_void)> {
    Some(xaac_free)
}
