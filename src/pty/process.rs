use crate::pty::master::PtyMaster;
use crate::pty::slave::PtySlave;

#[derive(Debug)]
pub enum PtyError {
    Fork(String),
}

pub fn spawn_shell() -> Result<PtyMaster, PtyError> {
    Ok(PtyMaster {})
}

pub fn fork_pty() -> Result<(PtyMaster, PtySlave), PtyError> {
    Ok((PtyMaster {}, PtySlave {}))
}

pub fn kill(_pid: i32) {
    // stub
}

pub fn wait_exit(_pid: i32) -> Result<i32, PtyError> {
    Ok(0)
}
