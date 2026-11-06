use smallvec::SmallVec;
use std::collections::HashMap;
use std::os::unix::io::{BorrowedFd, RawFd};
use wayland_client::backend::ObjectId;
use wayland_client::protocol::{wl_buffer, wl_shm, wl_shm_pool};
use wayland_client::{Dispatch, Proxy, QueueHandle};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferState {
    /// Buffer is free and ready to be acquired and drawn into.
    Available,
    /// Buffer has been acquired and is currently being written into.
    Acquired,
    /// Buffer was attached and committed; compositor owns it until release.
    CompositorOwned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcquireAction<Id> {
    /// An existing available buffer can be reused.
    Reuse(Id),
    /// A new buffer should be allocated.
    AllocateNew,
    /// All buffers in the pool are compositor-owned; frame must be deferred.
    NoAvailableBuffer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseAction {
    /// Buffer returned to Available state in the active pool.
    MakeAvailable,
    /// Buffer was retired (e.g. from resize); destroyed cleanly upon release.
    Destroy,
    /// Buffer was unknown or ignored.
    Ignored,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BufferSlot<Id> {
    pub id: Id,
    pub width: u32,
    pub height: u32,
    pub state: BufferState,
    pub pending_destroy: bool,
}

/// Generic, mockable state machine tracking buffer ownership, released-buffer reuse,
/// and safe resize/destruction lifecycles.
#[derive(Debug, Clone)]
pub struct BufferLifecycleState<Id: Clone + Eq + std::hash::Hash> {
    pub max_buffers: usize,
    pub width: u32,
    pub height: u32,
    pub active_buffers: SmallVec<[BufferSlot<Id>; 4]>,
    pub retired_buffers: SmallVec<[BufferSlot<Id>; 4]>,
}

impl<Id: Clone + Eq + std::hash::Hash> BufferLifecycleState<Id> {
    pub fn new(max_buffers: usize) -> Self {
        Self {
            max_buffers: max_buffers.max(1),
            width: 0,
            height: 0,
            active_buffers: SmallVec::new(),
            retired_buffers: SmallVec::new(),
        }
    }

    /// Acquires a buffer for rendering with dimensions `(width, height)`.
    /// Returns the acquisition action and a list of buffer IDs that were available and
    /// can be destroyed immediately (e.g. obsolete dimensions from resize).
    pub fn acquire(&mut self, width: u32, height: u32) -> (AcquireAction<Id>, Vec<Id>) {
        let mut immediate_destroys = Vec::new();

        // 1. Detect dimension changes (window resize)
        if (width != self.width || height != self.height) && (self.width != 0 && self.height != 0) {
            for mut slot in self.active_buffers.drain(..) {
                if slot.state == BufferState::Available {
                    // Compositor does not reference this buffer; destroy immediately
                    immediate_destroys.push(slot.id);
                } else {
                    // Compositor still owns this buffer! Move to retired list and hold until release.
                    slot.pending_destroy = true;
                    self.retired_buffers.push(slot);
                }
            }
            self.width = width;
            self.height = height;
        } else if self.width == 0 && self.height == 0 {
            self.width = width;
            self.height = height;
        }

        // 2. Search for an available buffer in active_buffers matching current dimensions
        for slot in &mut self.active_buffers {
            if slot.state == BufferState::Available && slot.width == width && slot.height == height
            {
                slot.state = BufferState::Acquired;
                return (AcquireAction::Reuse(slot.id.clone()), immediate_destroys);
            }
        }

        // 3. No available buffer found. Can we allocate a new one within max_buffers?
        if self.active_buffers.len() < self.max_buffers {
            (AcquireAction::AllocateNew, immediate_destroys)
        } else {
            // All buffers in the pool are CompositorOwned. Defer frame!
            (AcquireAction::NoAvailableBuffer, immediate_destroys)
        }
    }

    /// Registers a newly allocated buffer in the active pool as Acquired.
    pub fn register_new(&mut self, id: Id, width: u32, height: u32) {
        self.active_buffers.push(BufferSlot {
            id,
            width,
            height,
            state: BufferState::Acquired,
            pending_destroy: false,
        });
    }

    /// Marks an acquired buffer as submitted (attached & committed; compositor-owned).
    pub fn submit(&mut self, id: &Id) {
        if let Some(slot) = self.active_buffers.iter_mut().find(|s| &s.id == id) {
            slot.state = BufferState::CompositorOwned;
        }
    }

    /// Handles a buffer release event from the compositor.
    pub fn on_release(&mut self, id: &Id) -> ReleaseAction {
        // 1. Check retired buffers (from previous dimensions)
        if let Some(pos) = self.retired_buffers.iter().position(|s| &s.id == id) {
            self.retired_buffers.swap_remove(pos);
            return ReleaseAction::Destroy;
        }

        // 2. Check active buffers
        if let Some(pos) = self.active_buffers.iter().position(|s| &s.id == id) {
            if self.active_buffers[pos].pending_destroy {
                self.active_buffers.swap_remove(pos);
                return ReleaseAction::Destroy;
            } else {
                self.active_buffers[pos].state = BufferState::Available;
                return ReleaseAction::MakeAvailable;
            }
        }

        ReleaseAction::Ignored
    }

    #[inline(always)]
    pub fn available_count(&self) -> usize {
        self.active_buffers
            .iter()
            .filter(|s| s.state == BufferState::Available)
            .count()
    }

    #[inline(always)]
    pub fn compositor_owned_count(&self) -> usize {
        self.active_buffers
            .iter()
            .filter(|s| s.state == BufferState::CompositorOwned)
            .count()
            + self.retired_buffers.len()
    }

    #[inline(always)]
    pub fn retired_count(&self) -> usize {
        self.retired_buffers.len()
    }

    #[inline(always)]
    pub fn active_count(&self) -> usize {
        self.active_buffers.len()
    }
}

pub const MAX_WAYLAND_BUFFER_DIMENSION: u32 = 16_384;

/// Maximum total memory size allowed for a Wayland SHM buffer (256 MiB safe limit).
/// E.g., an 8K (7680x4320) 32-bpp buffer is ~126.5 MiB, well within this limit.
pub const MAX_WAYLAND_BUFFER_BYTES: usize = 256 * 1024 * 1024;

/// Structured, descriptive errors encountered during Wayland SHM buffer allocation and lifecycle.
#[derive(Debug)]
pub enum WaylandBufferError {
    ZeroDimension { width: u32, height: u32 },
    DimensionTooLarge { width: u32, height: u32, max: u32 },
    ArithmeticOverflow(&'static str),
    BufferTooLarge { size: usize, max: usize },
    ProtocolLimitExceeded(&'static str),
    MemfdCreate(std::io::Error),
    Ftruncate { size: usize, source: std::io::Error },
    Mmap { size: usize, source: std::io::Error },
    PoolCreationFailed,
    BufferCreationFailed,
}

impl std::fmt::Display for WaylandBufferError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ZeroDimension { width, height } => {
                write!(
                    f,
                    "Invalid Wayland buffer dimensions ({width}x{height}): dimensions must be positive"
                )
            }
            Self::DimensionTooLarge { width, height, max } => {
                write!(
                    f,
                    "Wayland buffer dimensions ({width}x{height}) exceed maximum allowed dimension ({max})"
                )
            }
            Self::ArithmeticOverflow(desc) => {
                write!(
                    f,
                    "Arithmetic overflow calculating Wayland buffer parameter: {desc}"
                )
            }
            Self::BufferTooLarge { size, max } => {
                write!(
                    f,
                    "Wayland buffer size {size} bytes exceeds maximum framebuffer limit {max} bytes"
                )
            }
            Self::ProtocolLimitExceeded(desc) => {
                write!(
                    f,
                    "Wayland buffer parameter exceeds i32 protocol limit: {desc}"
                )
            }
            Self::MemfdCreate(err) => {
                write!(f, "Failed to create memfd for Wayland buffer: {err}")
            }
            Self::Ftruncate { size, source } => {
                write!(f, "Failed to ftruncate memfd to {size} bytes: {source}")
            }
            Self::Mmap { size, source } => {
                write!(f, "Failed to mmap memfd of {size} bytes: {source}")
            }
            Self::PoolCreationFailed => {
                write!(f, "Failed to create Wayland shm pool")
            }
            Self::BufferCreationFailed => {
                write!(f, "Failed to create Wayland buffer from shm pool")
            }
        }
    }
}

impl std::error::Error for WaylandBufferError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::MemfdCreate(e) => Some(e),
            Self::Ftruncate { source, .. } => Some(source),
            Self::Mmap { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Pre-calculated and validated layout parameters for a Wayland SHM buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BufferLayout {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub pixel_count: usize,
    pub size_bytes: usize,
    pub i32_width: i32,
    pub i32_height: i32,
    pub i32_stride: i32,
    pub i32_size: i32,
}

/// Validates dimensions and performs checked arithmetic for stride, pixel count, byte size,
/// safe framebuffer limits, and Wayland protocol `i32` field bounds.
pub fn validate_buffer_dimensions(
    width: u32,
    height: u32,
) -> Result<BufferLayout, WaylandBufferError> {
    if width == 0 || height == 0 {
        return Err(WaylandBufferError::ZeroDimension { width, height });
    }

    if width > MAX_WAYLAND_BUFFER_DIMENSION || height > MAX_WAYLAND_BUFFER_DIMENSION {
        return Err(WaylandBufferError::DimensionTooLarge {
            width,
            height,
            max: MAX_WAYLAND_BUFFER_DIMENSION,
        });
    }

    let stride = width
        .checked_mul(4)
        .ok_or(WaylandBufferError::ArithmeticOverflow("stride (width * 4)"))?;

    let pixel_count_u32 =
        width
            .checked_mul(height)
            .ok_or(WaylandBufferError::ArithmeticOverflow(
                "pixel count (width * height)",
            ))?;
    let pixel_count = usize::try_from(pixel_count_u32)
        .map_err(|_| WaylandBufferError::ArithmeticOverflow("pixel count to usize"))?;

    let stride_usize = usize::try_from(stride)
        .map_err(|_| WaylandBufferError::ArithmeticOverflow("stride to usize"))?;
    let height_usize = usize::try_from(height)
        .map_err(|_| WaylandBufferError::ArithmeticOverflow("height to usize"))?;
    let size_bytes =
        stride_usize
            .checked_mul(height_usize)
            .ok_or(WaylandBufferError::ArithmeticOverflow(
                "buffer byte size (stride * height)",
            ))?;

    if size_bytes > MAX_WAYLAND_BUFFER_BYTES {
        return Err(WaylandBufferError::BufferTooLarge {
            size: size_bytes,
            max: MAX_WAYLAND_BUFFER_BYTES,
        });
    }

    let i32_width = i32::try_from(width)
        .map_err(|_| WaylandBufferError::ProtocolLimitExceeded("width exceeds i32::MAX"))?;
    let i32_height = i32::try_from(height)
        .map_err(|_| WaylandBufferError::ProtocolLimitExceeded("height exceeds i32::MAX"))?;
    let i32_stride = i32::try_from(stride)
        .map_err(|_| WaylandBufferError::ProtocolLimitExceeded("stride exceeds i32::MAX"))?;
    let i32_size = i32::try_from(size_bytes)
        .map_err(|_| WaylandBufferError::ProtocolLimitExceeded("size_bytes exceeds i32::MAX"))?;

    Ok(BufferLayout {
        width,
        height,
        stride,
        pixel_count,
        size_bytes,
        i32_width,
        i32_height,
        i32_stride,
        i32_size,
    })
}

/// RAII guard ensuring all intermediate resources (fd, mmap, pool, buffer)
/// are automatically cleaned up if initialization fails or panics before completion.
pub struct ShmBufferInitGuard {
    pub fd: RawFd,
    pub mem: *mut libc::c_void,
    pub size_bytes: usize,
    pub pool: Option<wl_shm_pool::WlShmPool>,
    pub buffer: Option<wl_buffer::WlBuffer>,
}

impl Drop for ShmBufferInitGuard {
    fn drop(&mut self) {
        if let Some(buffer) = self.buffer.take() {
            buffer.destroy();
        }
        if let Some(pool) = self.pool.take() {
            pool.destroy();
        }
        if !self.mem.is_null() && self.mem != libc::MAP_FAILED {
            let ret = unsafe { libc::munmap(self.mem, self.size_bytes) };
            if ret < 0 {
                log::warn!(
                    "ShmBufferInitGuard munmap failed: {}",
                    std::io::Error::last_os_error()
                );
            }
            self.mem = std::ptr::null_mut();
        }
        if self.fd >= 0 {
            let ret = unsafe { libc::close(self.fd) };
            if ret < 0 {
                let err = std::io::Error::last_os_error();
                if err.raw_os_error() != Some(libc::EBADF) {
                    log::warn!("ShmBufferInitGuard close fd {} failed: {}", self.fd, err);
                }
            }
            self.fd = -1;
        }
    }
}

pub struct WaylandShmBuffer {
    pub pool: wl_shm_pool::WlShmPool,
    pub buffer: wl_buffer::WlBuffer,
    pub mem: *mut u32,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub pixel_count: usize,
    pub size_bytes: usize,
    pub fd: RawFd,
}

impl WaylandShmBuffer {
    pub fn new<D>(
        shm: &wl_shm::WlShm,
        qh: &QueueHandle<D>,
        width: u32,
        height: u32,
    ) -> Result<Self, WaylandBufferError>
    where
        D: Dispatch<wl_shm_pool::WlShmPool, ()> + Dispatch<wl_buffer::WlBuffer, ()> + 'static,
    {
        let layout = validate_buffer_dimensions(width, height)?;

        let mut guard = ShmBufferInitGuard {
            fd: -1,
            mem: std::ptr::null_mut(),
            size_bytes: layout.size_bytes,
            pool: None,
            buffer: None,
        };

        let fd = unsafe {
            let name = b"velox-shm\0";
            libc::memfd_create(name.as_ptr() as *const libc::c_char, libc::MFD_CLOEXEC)
        };
        if fd < 0 {
            return Err(WaylandBufferError::MemfdCreate(
                std::io::Error::last_os_error(),
            ));
        }
        guard.fd = fd;

        let trunc_res = unsafe { libc::ftruncate(fd, layout.size_bytes as libc::off_t) };
        if trunc_res < 0 {
            return Err(WaylandBufferError::Ftruncate {
                size: layout.size_bytes,
                source: std::io::Error::last_os_error(),
            });
        }

        let mem = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                layout.size_bytes,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                fd,
                0,
            )
        };
        if mem == libc::MAP_FAILED || mem.is_null() {
            return Err(WaylandBufferError::Mmap {
                size: layout.size_bytes,
                source: std::io::Error::last_os_error(),
            });
        }
        guard.mem = mem;

        let borrowed_fd = unsafe { BorrowedFd::borrow_raw(fd) };
        let pool = shm.create_pool(borrowed_fd, layout.i32_size, qh, ());
        guard.pool = Some(pool);

        let buffer = guard.pool.as_ref().unwrap().create_buffer(
            0,
            layout.i32_width,
            layout.i32_height,
            layout.i32_stride,
            wl_shm::Format::Argb8888,
            qh,
            (),
        );
        guard.buffer = Some(buffer);

        // Disarm guard and transfer ownership to WaylandShmBuffer
        let final_fd = guard.fd;
        let final_mem = guard.mem;
        guard.fd = -1;
        guard.mem = std::ptr::null_mut();
        let final_pool = guard.pool.take().expect("pool was set");
        let final_buffer = guard.buffer.take().expect("buffer was set");

        Ok(Self {
            pool: final_pool,
            buffer: final_buffer,
            mem: final_mem as *mut u32,
            width: layout.width,
            height: layout.height,
            stride: layout.stride,
            pixel_count: layout.pixel_count,
            size_bytes: layout.size_bytes,
            fd: final_fd,
        })
    }

    #[inline(always)]
    pub fn pixels_mut(&mut self) -> &mut [u32] {
        unsafe { std::slice::from_raw_parts_mut(self.mem, self.pixel_count) }
    }
}

impl Drop for WaylandShmBuffer {
    fn drop(&mut self) {
        self.buffer.destroy();
        self.pool.destroy();
        if !self.mem.is_null() && self.mem != (libc::MAP_FAILED as *mut u32) {
            let ret = unsafe { libc::munmap(self.mem as *mut libc::c_void, self.size_bytes) };
            if ret < 0 {
                log::warn!(
                    "WaylandShmBuffer munmap failed: {}",
                    std::io::Error::last_os_error()
                );
            }
            self.mem = std::ptr::null_mut();
        }
        if self.fd >= 0 {
            let ret = unsafe { libc::close(self.fd) };
            if ret < 0 {
                let err = std::io::Error::last_os_error();
                if err.raw_os_error() != Some(libc::EBADF) {
                    log::warn!("WaylandShmBuffer close fd {} failed: {}", self.fd, err);
                }
            }
            self.fd = -1;
        }
    }
}

/// Triple-buffering Wayland SHM pool managing buffer acquisition, submission, release,
/// and safe resize lifecycles without tearing or invalid-pointer access.
pub struct WaylandBufferPool {
    pub lifecycle: BufferLifecycleState<ObjectId>,
    buffers: HashMap<ObjectId, WaylandShmBuffer>,
}

impl WaylandBufferPool {
    pub fn new(max_buffers: usize) -> Self {
        Self {
            lifecycle: BufferLifecycleState::new(max_buffers),
            buffers: HashMap::new(),
        }
    }

    /// Acquires an available buffer of given dimensions, or allocates a new one if pool limit is not reached.
    /// If all buffers are compositor-owned, returns `None` so the caller can defer the frame.
    pub fn acquire<D>(
        &mut self,
        shm: &wl_shm::WlShm,
        qh: &QueueHandle<D>,
        width: u32,
        height: u32,
    ) -> Option<&mut WaylandShmBuffer>
    where
        D: Dispatch<wl_shm_pool::WlShmPool, ()> + Dispatch<wl_buffer::WlBuffer, ()> + 'static,
    {
        let (action, immediate_destroys) = self.lifecycle.acquire(width, height);

        // Destroy any obsolete available buffers immediately
        for id in immediate_destroys {
            self.buffers.remove(&id);
        }

        match action {
            AcquireAction::Reuse(id) => self.buffers.get_mut(&id),
            AcquireAction::AllocateNew => match WaylandShmBuffer::new(shm, qh, width, height) {
                Ok(new_buf) => {
                    let id = new_buf.buffer.id();
                    self.lifecycle.register_new(id.clone(), width, height);
                    self.buffers.insert(id.clone(), new_buf);
                    self.buffers.get_mut(&id)
                }
                Err(e) => {
                    log::error!("Failed to allocate new WaylandShmBuffer: {}", e);
                    None
                }
            },
            AcquireAction::NoAvailableBuffer => None,
        }
    }

    /// Submits a buffer (marks it as owned by the compositor).
    pub fn submit(&mut self, id: &ObjectId) {
        self.lifecycle.submit(id);
    }

    /// Handles a buffer release event from the compositor.
    pub fn release(&mut self, buffer: &wl_buffer::WlBuffer) -> ReleaseAction {
        let action = self.lifecycle.on_release(&buffer.id());
        if action == ReleaseAction::Destroy {
            self.buffers.remove(&buffer.id());
        }
        action
    }

    pub fn is_empty(&self) -> bool {
        self.buffers.is_empty()
    }

    pub fn len(&self) -> usize {
        self.buffers.len()
    }

    pub fn lifecycle(&self) -> &BufferLifecycleState<ObjectId> {
        &self.lifecycle
    }
}
