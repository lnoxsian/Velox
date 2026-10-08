use std::ptr;
use x11rb::connection::Connection;
use x11rb::protocol::shm::{self, ConnectionExt as ShmConnectionExt};
use x11rb::protocol::xproto::{
    self, ConnectionExt as XprotoConnectionExt, Drawable, Gcontext, Visualid,
};
use x11rb::rust_connection::RustConnection;

/// Manages an X11 MIT-SHM shared memory segment or fallbacks to standard put_image.
pub struct X11Framebuffer {
    pub width: u32,
    pub height: u32,
    pub depth: u8,
    shm_seg: Option<shm::Seg>,
    shm_id: i32,
    shm_addr: *mut u32,
    pub buffer: Vec<u32>,
    has_shm: bool,
}

impl X11Framebuffer {
    pub fn new(conn: &RustConnection, width: u32, height: u32, depth: u8) -> Self {
        let has_shm = conn
            .shm_query_version()
            .ok()
            .and_then(|cookie| cookie.reply().ok())
            .is_some();

        let mut fb = Self {
            width: 0,
            height: 0,
            depth,
            shm_seg: None,
            shm_id: -1,
            shm_addr: ptr::null_mut(),
            buffer: Vec::new(),
            has_shm,
        };
        fb.resize(conn, width, height);
        fb
    }

    pub fn resize(&mut self, conn: &RustConnection, width: u32, height: u32) {
        let w = width.max(1);
        let h = height.max(1);
        if self.width == w && self.height == h {
            return;
        }

        self.cleanup_shm(conn);
        self.width = w;
        self.height = h;
        let num_pixels = (w * h) as usize;
        let num_bytes = num_pixels * 4;

        if self.has_shm {
            unsafe {
                let shm_id = libc::shmget(libc::IPC_PRIVATE, num_bytes, libc::IPC_CREAT | 0o600);
                if shm_id >= 0 {
                    let addr = libc::shmat(shm_id, ptr::null(), 0);
                    if addr != (-1isize as *mut libc::c_void) {
                        let seg = conn.generate_id().unwrap_or(0);
                        if conn.shm_attach(seg, shm_id as u32, false).is_ok() {
                            let _ = conn.flush();
                            self.shm_seg = Some(seg);
                            self.shm_id = shm_id;
                            self.shm_addr = addr as *mut u32;
                            return;
                        }
                        libc::shmdt(addr);
                    }
                    libc::shmctl(shm_id, libc::IPC_RMID, ptr::null_mut());
                }
            }
        }

        // Fallback to heap buffer
        self.buffer.resize(num_pixels, 0);
    }

    pub fn pixels_mut(&mut self) -> &mut [u32] {
        if !self.shm_addr.is_null() {
            unsafe {
                std::slice::from_raw_parts_mut(self.shm_addr, (self.width * self.height) as usize)
            }
        } else {
            &mut self.buffer
        }
    }

    pub fn present(
        &self,
        conn: &RustConnection,
        drawable: Drawable,
        gc: Gcontext,
        _visual: Visualid,
    ) {
        if let Some(seg) = self.shm_seg {
            let _ = conn.shm_put_image(
                drawable,
                gc,
                self.width as u16,
                self.height as u16,
                0,
                0,
                self.width as u16,
                self.height as u16,
                0,
                0,
                self.depth,
                xproto::ImageFormat::Z_PIXMAP.into(),
                false,
                seg,
                0,
            );
        } else {
            let bytes = unsafe {
                std::slice::from_raw_parts(self.buffer.as_ptr() as *const u8, self.buffer.len() * 4)
            };
            let _ = conn.put_image(
                xproto::ImageFormat::Z_PIXMAP,
                drawable,
                gc,
                self.width as u16,
                self.height as u16,
                0,
                0,
                0,
                self.depth,
                bytes,
            );
        }
        let _ = conn.flush();
    }

    fn cleanup_shm(&mut self, conn: &RustConnection) {
        if let Some(seg) = self.shm_seg.take() {
            let _ = conn.shm_detach(seg);
            let _ = conn.flush();
        }
        if !self.shm_addr.is_null() {
            unsafe {
                libc::shmdt(self.shm_addr as *mut libc::c_void);
            }
            self.shm_addr = ptr::null_mut();
        }
        if self.shm_id >= 0 {
            unsafe {
                libc::shmctl(self.shm_id, libc::IPC_RMID, ptr::null_mut());
            }
            self.shm_id = -1;
        }
    }
}

impl Drop for X11Framebuffer {
    fn drop(&mut self) {
        if !self.shm_addr.is_null() {
            unsafe {
                libc::shmdt(self.shm_addr as *mut libc::c_void);
            }
            self.shm_addr = ptr::null_mut();
        }
        if self.shm_id >= 0 {
            unsafe {
                libc::shmctl(self.shm_id, libc::IPC_RMID, ptr::null_mut());
            }
        }
    }
}
