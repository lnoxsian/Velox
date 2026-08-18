/// Platform-specific memory trimming to release unused allocator pages back to the operating system.
#[cfg(target_os = "linux")]
#[inline]
pub fn trim_allocator_memory() {
    unsafe {
        libc::malloc_trim(0);
    }
}

#[cfg(not(target_os = "linux"))]
#[inline]
pub fn trim_allocator_memory() {}
