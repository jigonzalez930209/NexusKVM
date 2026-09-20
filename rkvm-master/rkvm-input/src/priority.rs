use std::io;

/// Result of the kernel-level latency boost applied at startup.
pub struct Boost {
    /// Some(prio) when SCHED_FIFO realtime was granted (needs CAP_SYS_NICE).
    pub rt_prio: Option<i32>,
    /// Nice value when falling back to normal scheduling (-20 best effort).
    pub nice: i32,
    /// Whether memory got locked via mlockall (no pageout jitter).
    pub memlocked: bool,
}

/// Raise CPU attention to kernel level for low-latency input forwarding:
///
/// 1. `SCHED_FIFO` realtime scheduling — the input threads stop competing
///    with desktop load and microdelays from preemption disappear. Needs
///    CAP_SYS_NICE (root services get it; run `sudo setcap cap_sys_nice+ep`
///    on the binaries otherwise).
/// 2. Fallback: best-effort negative nice.
/// 3. `mlockall(MCL_CURRENT)` — already-mapped pages never swap out, killing
///    soft page-fault stalls. `MCL_FUTURE` is deliberately not used: it makes
///    every later allocation fail once RLIMIT_MEMLOCK is exhausted, and Rust's
///    allocation error handler aborts the process.
pub fn boost_cpu() -> Boost {
    let mut out = boost_thread();
    out.memlocked = lock_memory();
    out
}

/// Per-thread boost for tokio worker threads (no mlockall, no logging).
///
/// Must run on each worker: `sched_setscheduler(0, ...)` only affects the
/// calling thread, so boosting just the main thread leaves the input tasks on
/// SCHED_OTHER.
pub fn boost_thread() -> Boost {
    let mut out = Boost {
        rt_prio: None,
        nice: 0,
        memlocked: false,
    };

    // Realtime first: descending priorities stay below critical system RT
    // tasks (kernel threads ~99, PulseAudio ~88) but above everything normal.
    for prio in [80, 70, 60, 50, 40, 30, 20] {
        let param = libc::sched_param {
            sched_priority: prio,
        };
        let rc = unsafe { libc::sched_setscheduler(0, libc::SCHED_FIFO, &param) };
        if rc == 0 {
            out.rt_prio = Some(prio);
            break;
        }
    }

    // Best-effort nice fallback (also useful when FIFO fails midway).
    for nice in [-20i32, -15, -10, -5, 0] {
        let rc = unsafe { libc::setpriority(libc::PRIO_PROCESS, 0, nice) };
        if rc == 0 {
            out.nice = nice;
            break;
        }
    }

    out
}

fn lock_memory() -> bool {
    let rc = unsafe { libc::mlockall(libc::MCL_CURRENT) };
    if rc != 0 {
        let err = io::Error::last_os_error();
        eprintln!("mlockall failed ({err}); continuing without memory pinning");
        return false;
    }
    true
}
