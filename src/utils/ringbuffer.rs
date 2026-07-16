pub struct RingBuffer<T> {
    pub buffer: Vec<T>,
}

impl<T> RingBuffer<T> {
    pub fn new() -> Self {
        Self { buffer: Vec::new() }
    }
}

impl<T> Default for RingBuffer<T> {
    fn default() -> Self {
        Self::new()
    }
}
