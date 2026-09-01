use std::sync::{Mutex, OnceLock};

pub const PTY_BUFFER_SIZE: usize = 65536;
pub const MAX_POOLED_BUFFERS: usize = 32;

/// Reusable buffer pool for PTY output reading, avoiding per-read heap allocations.
pub struct PtyBufferPool {
    pool: Mutex<Vec<Vec<u8>>>,
}

impl PtyBufferPool {
    pub fn new() -> Self {
        Self {
            pool: Mutex::new(Vec::with_capacity(MAX_POOLED_BUFFERS)),
        }
    }

    #[inline]
    pub fn acquire(&self) -> Vec<u8> {
        if let Ok(mut lock) = self.pool.lock()
            && let Some(mut buf) = lock.pop()
        {
            buf.clear();
            buf.resize(PTY_BUFFER_SIZE, 0);
            return buf;
        }
        vec![0u8; PTY_BUFFER_SIZE]
    }

    #[inline]
    pub fn recycle(&self, mut buf: Vec<u8>) {
        if buf.capacity() >= PTY_BUFFER_SIZE
            && let Ok(mut lock) = self.pool.lock()
            && lock.len() < MAX_POOLED_BUFFERS
        {
            buf.clear();
            lock.push(buf);
        }
    }

    pub fn len(&self) -> usize {
        self.pool.lock().map_or(0, |p| p.len())
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for PtyBufferPool {
    fn default() -> Self {
        Self::new()
    }
}

static GLOBAL_PTY_POOL: OnceLock<PtyBufferPool> = OnceLock::new();

#[inline(always)]
pub fn get_pty_buffer_pool() -> &'static PtyBufferPool {
    GLOBAL_PTY_POOL.get_or_init(PtyBufferPool::new)
}

#[inline(always)]
pub fn acquire_pty_buffer() -> Vec<u8> {
    get_pty_buffer_pool().acquire()
}

#[inline(always)]
pub fn recycle_pty_buffer(buf: Vec<u8>) {
    get_pty_buffer_pool().recycle(buf);
}
