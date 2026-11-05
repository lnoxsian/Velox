use std::ptr;
use x11rb::connection::Connection;
use x11rb::protocol::shm::{self, ConnectionExt as ShmConnectionExt};
use x11rb::protocol::xproto::{
    self, ConnectionExt as XprotoConnectionExt, Drawable, Gcontext, Visualid,
};
use x11rb::rust_connection::RustConnection;

/// Maximum single dimension (width or height) allowed for an X11 framebuffer (16K resolution limit).
pub const MAX_X11_BUFFER_DIMENSION: u32 = 16_384;

/// Maximum total memory size allowed for an X11 framebuffer (256 MiB safe limit).
/// E.g., an 8K (7680x4320) 32-bpp buffer is ~126.5 MiB, well within this limit.
pub const MAX_X11_BUFFER_BYTES: usize = 256 * 1024 * 1024;

/// Structured, descriptive errors encountered during X11 framebuffer allocation and resize.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum X11BufferError {
    ZeroDimension { width: u32, height: u32 },
    DimensionTooLarge { width: u32, height: u32, max: u32 },
    ArithmeticOverflow(&'static str),
    BufferTooLarge { size: usize, max: usize },
    AllocationFailed(&'static str),
}

impl std::fmt::Display for X11BufferError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ZeroDimension { width, height } => {
                write!(
                    f,
                    "Invalid X11 framebuffer dimensions ({width}x{height}): dimensions must be positive"
                )
            }
            Self::DimensionTooLarge { width, height, max } => {
                write!(
                    f,
                    "X11 framebuffer dimensions ({width}x{height}) exceed maximum allowed dimension ({max})"
                )
            }
            Self::ArithmeticOverflow(desc) => {
                write!(
                    f,
                    "Arithmetic overflow calculating X11 framebuffer size: {desc}"
                )
            }
            Self::BufferTooLarge { size, max } => {
                write!(
                    f,
                    "X11 framebuffer size {size} bytes exceeds maximum allowed limit {max} bytes"
                )
            }
            Self::AllocationFailed(reason) => {
                write!(f, "X11 framebuffer allocation failed: {reason}")
            }
        }
    }
}

impl std::error::Error for X11BufferError {}

/// Pre-calculated and validated layout parameters for an X11 framebuffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X11BufferLayout {
    pub width: u32,
    pub height: u32,
    pub pixel_count: usize,
    pub num_bytes: usize,
    pub alloc_bytes: usize,
}

/// Validates dimensions and performs checked arithmetic for pixel count, byte size,
/// safe framebuffer limits, and allocation headroom.
pub fn validate_x11_buffer_dimensions(
    width: u32,
    height: u32,
) -> Result<X11BufferLayout, X11BufferError> {
    if width == 0 || height == 0 {
        return Err(X11BufferError::ZeroDimension { width, height });
    }

    if width > MAX_X11_BUFFER_DIMENSION || height > MAX_X11_BUFFER_DIMENSION {
        return Err(X11BufferError::DimensionTooLarge {
            width,
            height,
            max: MAX_X11_BUFFER_DIMENSION,
        });
    }

    let w_usize =
        usize::try_from(width).map_err(|_| X11BufferError::ArithmeticOverflow("width to usize"))?;
    let h_usize = usize::try_from(height)
        .map_err(|_| X11BufferError::ArithmeticOverflow("height to usize"))?;

    let pixel_count = w_usize
        .checked_mul(h_usize)
        .ok_or(X11BufferError::ArithmeticOverflow(
            "width * height (pixels)",
        ))?;

    let num_bytes = pixel_count
        .checked_mul(4)
        .ok_or(X11BufferError::ArithmeticOverflow(
            "pixel_count * 4 (bytes)",
        ))?;

    if num_bytes > MAX_X11_BUFFER_BYTES {
        return Err(X11BufferError::BufferTooLarge {
            size: num_bytes,
            max: MAX_X11_BUFFER_BYTES,
        });
    }

    // 25% extra headroom for smooth window resizing without thrashing SHM allocations
    let alloc_bytes = num_bytes
        .checked_mul(5)
        .map(|v| v / 4)
        .unwrap_or(num_bytes)
        .min(MAX_X11_BUFFER_BYTES)
        .max(num_bytes);

    Ok(X11BufferLayout {
        width,
        height,
        pixel_count,
        num_bytes,
        alloc_bytes,
    })
}

/// RAII guard ensuring SysV shared memory segment and mapped address are cleaned up
/// exactly once if attachment fails, encounters an error, or panics before completion.
pub struct ShmSegmentGuard {
    pub shm_id: i32,
    pub addr: *mut libc::c_void,
}

impl Drop for ShmSegmentGuard {
    fn drop(&mut self) {
        if !self.addr.is_null() && self.addr != (-1isize as *mut libc::c_void) {
            unsafe {
                libc::shmdt(self.addr);
            }
            self.addr = ptr::null_mut();
        }
        if self.shm_id >= 0 {
            unsafe {
                libc::shmctl(self.shm_id, libc::IPC_RMID, ptr::null_mut());
            }
            self.shm_id = -1;
        }
    }
}

/// Manages an X11 MIT-SHM shared memory segment or fallbacks to standard put_image.
pub struct X11Framebuffer {
    pub width: u32,
    pub height: u32,
    pub depth: u8,
    pub(crate) shm_seg: Option<shm::Seg>,
    pub(crate) shm_id: i32,
    pub(crate) shm_addr: *mut u32,
    pub(crate) shm_capacity_bytes: usize,
    pub pixel_count: usize,
    pub buffer: Vec<u32>,
    pub(crate) has_shm: bool,
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
            shm_capacity_bytes: 0,
            pixel_count: 0,
            buffer: Vec::new(),
            has_shm,
        };
        if let Err(e) = fb.resize(conn, width, height) {
            log::warn!(
                "Initial X11 framebuffer allocation for ({width}x{height}) failed: {e}; falling back to 1x1."
            );
            let _ = fb.resize(conn, 1, 1);
        }
        fb
    }

    /// Creates a purely software (heap-backed) X11 framebuffer without shared memory.
    pub fn new_software(width: u32, height: u32, depth: u8) -> Result<Self, X11BufferError> {
        let layout = validate_x11_buffer_dimensions(width, height)?;
        let mut buffer = Vec::new();
        if buffer.try_reserve_exact(layout.pixel_count).is_err() {
            return Err(X11BufferError::AllocationFailed(
                "heap allocation failed for software framebuffer",
            ));
        }
        buffer.resize(layout.pixel_count, 0);

        Ok(Self {
            width: layout.width,
            height: layout.height,
            depth,
            shm_seg: None,
            shm_id: -1,
            shm_addr: ptr::null_mut(),
            shm_capacity_bytes: 0,
            pixel_count: layout.pixel_count,
            buffer,
            has_shm: false,
        })
    }

    pub fn resize(
        &mut self,
        conn: &RustConnection,
        width: u32,
        height: u32,
    ) -> Result<(), X11BufferError> {
        let layout = match validate_x11_buffer_dimensions(width, height) {
            Ok(layout) => layout,
            Err(e) => {
                log::warn!(
                    "Invalid X11 framebuffer dimensions ({width}x{height}) rejected: {e}; retaining current buffer ({self_w}x{self_h})",
                    self_w = self.width,
                    self_h = self.height,
                );
                return Err(e);
            }
        };

        if self.width == layout.width && self.height == layout.height {
            return Ok(());
        }

        // If existing SHM segment is large enough, retain it! Zero SysV calls, zero X11 roundtrips!
        if self.has_shm && self.shm_seg.is_some() && layout.num_bytes <= self.shm_capacity_bytes {
            self.width = layout.width;
            self.height = layout.height;
            self.pixel_count = layout.pixel_count;
            return Ok(());
        }

        self.cleanup_shm(conn);

        let mut shm_success = false;
        if self.has_shm {
            let mut guard = ShmSegmentGuard {
                shm_id: -1,
                addr: ptr::null_mut(),
            };

            let shm_id = unsafe {
                libc::shmget(
                    libc::IPC_PRIVATE,
                    layout.alloc_bytes,
                    libc::IPC_CREAT | 0o600,
                )
            };

            if shm_id >= 0 {
                guard.shm_id = shm_id;
                let addr = unsafe { libc::shmat(shm_id, ptr::null(), 0) };
                if addr != (-1isize as *mut libc::c_void) {
                    guard.addr = addr;

                    if let Ok(seg) = conn.generate_id() {
                        let attach_res = conn
                            .shm_attach(seg, shm_id as u32, false)
                            .map_err(|e| e.to_string())
                            .and_then(|cookie| cookie.check().map_err(|e| e.to_string()));

                        match attach_res {
                            Ok(_) => {
                                let _ = conn.flush();
                                // Disarm guard: successfully attached and verified
                                guard.shm_id = -1;
                                guard.addr = ptr::null_mut();

                                self.shm_seg = Some(seg);
                                self.shm_id = shm_id;
                                self.shm_addr = addr as *mut u32;
                                self.shm_capacity_bytes = layout.alloc_bytes;
                                self.width = layout.width;
                                self.height = layout.height;
                                self.pixel_count = layout.pixel_count;
                                shm_success = true;
                            }
                            Err(e) => {
                                log::warn!(
                                    "X11 shm_attach failed: {e}. Disabling SHM and falling back to heap buffer."
                                );
                                self.has_shm = false;
                                // guard will drop here and clean up addr & shm_id exactly once
                            }
                        }
                    } else {
                        log::warn!(
                            "Failed to generate X11 ID for shm segment; falling back to heap buffer."
                        );
                    }
                } else {
                    log::warn!("libc::shmat failed; falling back to heap buffer.");
                }
            } else {
                log::warn!(
                    "libc::shmget failed for {} bytes; falling back to heap buffer.",
                    layout.alloc_bytes
                );
            }
        }

        if !shm_success {
            self.shm_capacity_bytes = 0;
            self.buffer.clear();
            if self.buffer.try_reserve_exact(layout.pixel_count).is_err() {
                return Err(X11BufferError::AllocationFailed(
                    "heap allocation failed during framebuffer fallback",
                ));
            }
            self.buffer.resize(layout.pixel_count, 0);
            self.width = layout.width;
            self.height = layout.height;
            self.pixel_count = layout.pixel_count;
        }

        Ok(())
    }

    #[inline(always)]
    pub fn pixels_mut(&mut self) -> &mut [u32] {
        if !self.shm_addr.is_null() {
            unsafe { std::slice::from_raw_parts_mut(self.shm_addr, self.pixel_count) }
        } else {
            &mut self.buffer
        }
    }

    pub fn present(
        &self,
        conn: &RustConnection,
        drawable: Drawable,
        gc: Gcontext,
        visual: Visualid,
    ) {
        self.present_damaged(conn, drawable, gc, visual, 0, self.height);
    }

    pub fn present_damaged(
        &self,
        conn: &RustConnection,
        drawable: Drawable,
        gc: Gcontext,
        _visual: Visualid,
        damaged_y: u32,
        damaged_height: u32,
    ) {
        if self.width == 0 || self.height == 0 || damaged_height == 0 {
            return;
        }

        let h = damaged_height.min(self.height.saturating_sub(damaged_y));
        if h == 0 {
            return;
        }

        let Ok(w_u16) = u16::try_from(self.width) else {
            return;
        };
        let Ok(h_u16) = u16::try_from(self.height) else {
            return;
        };
        let Ok(damaged_y_u16) = u16::try_from(damaged_y) else {
            return;
        };
        let Ok(damaged_y_i16) = i16::try_from(damaged_y) else {
            return;
        };
        let Ok(damaged_h_u16) = u16::try_from(h) else {
            return;
        };

        if let Some(seg) = self.shm_seg {
            let _ = conn.shm_put_image(
                drawable,
                gc,
                w_u16,
                h_u16,
                0,
                damaged_y_u16,
                w_u16,
                damaged_h_u16,
                0,
                damaged_y_i16,
                self.depth,
                xproto::ImageFormat::Z_PIXMAP.into(),
                false,
                seg,
                0,
            );
        } else {
            let stride = self.width as usize;
            let Some(start_px) = (damaged_y as usize).checked_mul(stride) else {
                return;
            };
            let Some(num_px) = (h as usize).checked_mul(stride) else {
                return;
            };
            let Some(end_px) = start_px.checked_add(num_px) else {
                return;
            };
            let end_px = end_px.min(self.buffer.len());

            if start_px < end_px {
                let Some(byte_len) = (end_px - start_px).checked_mul(4) else {
                    return;
                };
                let bytes = unsafe {
                    std::slice::from_raw_parts(
                        self.buffer[start_px..end_px].as_ptr() as *const u8,
                        byte_len,
                    )
                };
                let _ = conn.put_image(
                    xproto::ImageFormat::Z_PIXMAP,
                    drawable,
                    gc,
                    w_u16,
                    damaged_h_u16,
                    0,
                    damaged_y_i16,
                    0,
                    self.depth,
                    bytes,
                );
            }
        }
        let _ = conn.flush();
    }

    fn cleanup_shm(&mut self, conn: &RustConnection) {
        if let Some(seg) = self.shm_seg.take() {
            let _ = conn.shm_detach(seg);
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
        self.shm_capacity_bytes = 0;
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
            self.shm_id = -1;
        }
    }
}
