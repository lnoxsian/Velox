use std::os::unix::io::{BorrowedFd, RawFd};
use wayland_client::protocol::{wl_buffer, wl_shm, wl_shm_pool};
use wayland_client::{Dispatch, QueueHandle};

pub struct WaylandShmBuffer {
    pub pool: wl_shm_pool::WlShmPool,
    pub buffer: wl_buffer::WlBuffer,
    pub mem: *mut u32,
    pub width: u32,
    pub height: u32,
    pub size_bytes: usize,
    pub fd: RawFd,
}

impl WaylandShmBuffer {
    pub fn new<D>(
        shm: &wl_shm::WlShm,
        qh: &QueueHandle<D>,
        width: u32,
        height: u32,
    ) -> Result<Self, Box<dyn std::error::Error>>
    where
        D: Dispatch<wl_shm_pool::WlShmPool, ()> + Dispatch<wl_buffer::WlBuffer, ()> + 'static,
    {
        let w = width.max(1);
        let h = height.max(1);
        let stride = w * 4;
        let size_bytes = (stride * h) as usize;

        let fd = unsafe {
            let name = b"velox-shm\0";
            libc::memfd_create(name.as_ptr() as *const libc::c_char, libc::MFD_CLOEXEC)
        };
        if fd < 0 {
            return Err("Failed to allocate memfd for Wayland buffer".into());
        }

        unsafe {
            if libc::ftruncate(fd, size_bytes as libc::off_t) < 0 {
                libc::close(fd);
                return Err("Failed to truncate memfd".into());
            }

            let mem = libc::mmap(
                std::ptr::null_mut(),
                size_bytes,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                fd,
                0,
            );
            if mem == libc::MAP_FAILED {
                libc::close(fd);
                return Err("Failed to mmap memfd".into());
            }

            let borrowed_fd = BorrowedFd::borrow_raw(fd);
            let pool = shm.create_pool(borrowed_fd, size_bytes as i32, qh, ());
            let buffer = pool.create_buffer(
                0,
                w as i32,
                h as i32,
                stride as i32,
                wl_shm::Format::Argb8888,
                qh,
                (),
            );

            Ok(Self {
                pool,
                buffer,
                mem: mem as *mut u32,
                width: w,
                height: h,
                size_bytes,
                fd,
            })
        }
    }

    pub fn pixels_mut(&mut self) -> &mut [u32] {
        unsafe { std::slice::from_raw_parts_mut(self.mem, (self.width * self.height) as usize) }
    }
}

impl Drop for WaylandShmBuffer {
    fn drop(&mut self) {
        unsafe {
            if !self.mem.is_null() && self.mem != (libc::MAP_FAILED as *mut u32) {
                libc::munmap(self.mem as *mut libc::c_void, self.size_bytes);
            }
            if self.fd >= 0 {
                libc::close(self.fd);
            }
        }
        self.buffer.destroy();
        self.pool.destroy();
    }
}
