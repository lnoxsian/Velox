pub struct PtyMaster {
    // master fd
}

impl PtyMaster {
    pub fn read(&self, _buf: &mut [u8]) -> std::io::Result<usize> {
        Ok(0)
    }

    pub fn write(&self, _buf: &[u8]) -> std::io::Result<usize> {
        Ok(0)
    }

    pub fn resize(&self, _cols: u16, _rows: u16) -> std::io::Result<()> {
        Ok(())
    }
}
