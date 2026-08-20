use std::io;

/// Try to lower the process *nice* value (higher CPU priority).
/// Lower values = higher priority. Tries -20 … 0.
/// Without `CAP_SYS_NICE` / root it usually stays at `0` (normal priority).
pub fn raise_cpu() -> Result<i32, io::Error> {
    for nice in [-20i32, -15, -10, -5, 0] {
        let rc = unsafe { libc::setpriority(libc::PRIO_PROCESS, 0, nice) };
        if rc == 0 {
            return Ok(nice);
        }
    }
    Err(io::Error::last_os_error())
}
