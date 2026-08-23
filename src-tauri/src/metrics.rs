//! Real service-process metrics for the UI, read from /proc (Linux).
//!
//! The daemon/client are plain child processes of this app or systemd units
//! running the same binaries; both cases resolve to a PID via /proc scan.

use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::PathBuf,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServiceMetrics {
    pub pid: Option<u32>,
    pub service: Option<String>,
    pub cpu_percent: f32,
    pub mem_mb: f32,
    pub uptime_secs: u64,
}

struct ProcStat {
    utime: u64,
    stime: u64,
    starttime: u64,
    rss_pages: u64,
}

fn clk_tck() -> f64 {
    // USER_HZ; virtually always 100 on Linux. sysconf when available.
    let v = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
    if v <= 0 {
        100.0
    } else {
        v as f64
    }
}

fn page_size_bytes() -> f64 {
    let v = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    if v <= 0 {
        4096.0
    } else {
        v as f64
    }
}

fn cpu_cores() -> f64 {
    std::thread::available_parallelism()
        .map(|n| n.get() as f64)
        .unwrap_or(1.0)
}

/// Find the PID of a running service binary by matching /proc/<pid>/comm,
/// cmdline, or exe. Reading comm/cmdline is world-readable even across users.
pub fn find_service_pid(binary_name: &str) -> Option<u32> {
    let proc = PathBuf::from("/proc");
    let entries = fs::read_dir(&proc).ok()?;
    let mut best: Option<u32> = None;
    for entry in entries.flatten() {
        let Some(name) = entry.file_name().to_str().map(String::from) else {
            continue;
        };
        let Ok(pid) = name.parse::<u32>() else {
            continue;
        };
        if pid == 0 {
            continue;
        }
        let pid_dir = proc.join(&name);

        // 1. /proc/<pid>/comm (always world-readable, contains binary name)
        if let Ok(comm) = fs::read_to_string(pid_dir.join("comm")) {
            let comm_trim = comm.trim();
            if comm_trim == binary_name || binary_name.starts_with(comm_trim) {
                best = Some(best.map_or(pid, |prev: u32| prev.max(pid)));
                continue;
            }
        }

        // 2. /proc/<pid>/cmdline (arguments, always world-readable)
        if let Ok(cmdline) = fs::read_to_string(pid_dir.join("cmdline")) {
            if cmdline.contains(binary_name) {
                best = Some(best.map_or(pid, |prev: u32| prev.max(pid)));
                continue;
            }
        }

        // 3. /proc/<pid>/exe (symlink)
        if let Ok(exe) = fs::read_link(pid_dir.join("exe")) {
            if exe.file_name().and_then(|s| s.to_str()) == Some(binary_name) {
                best = Some(best.map_or(pid, |prev: u32| prev.max(pid)));
            }
        }
    }
    best
}

/// `/proc/<pid>/stat`: utime (14), stime (15), starttime (22); resident pages
/// from `/proc/<pid>/statm` field 2.
fn read_stat(pid: u32) -> Option<ProcStat> {
    let raw = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // comm may contain spaces/parens; everything after the last ')' is stable.
    let after = raw.rsplit_once(')')?.1;
    let fields: Vec<&str> = after.split_whitespace().collect();
    // After ')', field 3 (state) sits at index 0 → utime is index 11.
    let parse = |i: usize| fields.get(i).and_then(|v| v.parse::<u64>().ok());
    let rss_pages = fs::read_to_string(format!("/proc/{pid}/statm"))
        .ok()
        .and_then(|m| m.split_whitespace().nth(1).and_then(|v| v.parse().ok()))
        .unwrap_or(0);
    Some(ProcStat {
        utime: parse(11)?,
        stime: parse(12)?,
        starttime: parse(19)?,
        rss_pages,
    })
}

fn boot_time_secs() -> u64 {
    fs::read_to_string("/proc/stat")
        .ok()
        .and_then(|s| {
            s.lines().find_map(|l| {
                l.strip_prefix("btime")
                    .and_then(|r| r.trim().parse::<u64>().ok())
            })
        })
        .unwrap_or(0)
}

fn process_uptime_secs(starttime_ticks: u64, clk: f64) -> u64 {
    let started = boot_time_secs() as f64 + starttime_ticks as f64 / clk;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    ((now - started).max(0.0)) as u64
}

/// Lifetime-average CPU% — used before two samples exist or when the wall
/// delta between polls is too small to be meaningful.
fn lifetime_cpu(total_ticks: u64, starttime: u64, clk: f64) -> f64 {
    let uptime = process_uptime_secs(starttime, clk).max(1) as f64;
    (total_ticks as f64 / clk / uptime * 100.0).clamp(0.0, 100.0 * cpu_cores())
}

/// Rolling CPU sampler; keeps the previous tick counter to compute deltas
/// between UI polls (~2 s apart). First poll reports the lifetime average.
#[derive(Default)]
pub struct MetricsTracker {
    last: Option<(Instant, u64)>,
}

impl MetricsTracker {
    pub fn sample(&mut self, pid: Option<u32>, service: Option<&str>) -> ServiceMetrics {
        let mut out = ServiceMetrics {
            pid,
            service: service.map(String::from),
            ..Default::default()
        };
        let Some(pid) = pid else {
            self.last = None;
            return out;
        };
        let Some(stat) = read_stat(pid) else {
            self.last = None;
            return out;
        };

        let clk = clk_tck();
        let total_ticks = stat.utime + stat.stime;

        out.cpu_percent = match self.last {
            Some((t_prev, ticks_prev)) => {
                let wall = t_prev.elapsed().as_secs_f64();
                if wall < 0.2 {
                    lifetime_cpu(total_ticks, stat.starttime, clk) as f32
                } else {
                    (((total_ticks - ticks_prev) as f64 / clk / wall * 100.0)
                        .clamp(0.0, 100.0 * cpu_cores())) as f32
                }
            }
            None => lifetime_cpu(total_ticks, stat.starttime, clk) as f32,
        };
        self.last = Some((Instant::now(), total_ticks));

        out.mem_mb = (stat.rss_pages as f64 * page_size_bytes() / (1024.0 * 1024.0)) as f32;
        out.uptime_secs = process_uptime_secs(stat.starttime, clk);

        out
    }
}
