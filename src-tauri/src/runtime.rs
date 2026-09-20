pub use crate::metrics::ServiceMetrics;
use nexus_agent::daemon_client::DaemonClient;
use nexus_agent::layout_store::{self, AgentStatusFile};
use nexus_common::*;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    net::{IpAddr, UdpSocket},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
};
use tauri::{AppHandle, Manager};
use tokio::time::{sleep, Duration};

const LISTEN: &str = "0.0.0.0:5258";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Host,
    Client,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedState {
    pub role: Role,
    #[serde(default)]
    pub server: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invite {
    pub server: String,
    pub password: String,
    pub certificate: String,
    #[serde(default)]
    pub client_certificate: String,
    #[serde(default)]
    pub client_key: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeSnapshot {
    pub role: Option<Role>,
    pub running: bool,
    pub socket_ok: bool,
    pub service_ok: bool,
    pub listen: String,
    pub advertise: String,
    pub remote_server: Option<String>,
    #[serde(default)]
    pub password: String,
    pub has_password: bool,
    pub error: Option<String>,
    pub needs_logout: bool,
    pub log_dir: Option<String>,
    pub service_log: Option<String>,
    pub daemon: Option<AppStatus>,
    pub binary_host: Option<String>,
    pub binary_client: Option<String>,
    pub peer_side: Option<String>,
    pub portal_available: bool,
    pub portal_error: Option<String>,
    pub clipboard_ok: bool,
    #[serde(default)]
    pub metrics: ServiceMetrics,
}

#[derive(Default)]
struct Inner {
    daemon: Option<Child>,
    client: Option<Child>,
    agent: Option<Child>,
    last_error: Option<String>,
}

pub struct AppRuntime {
    inner: Mutex<Inner>,
    metrics: Mutex<crate::metrics::MetricsTracker>,
    /// False after an explicit user stop: the supervisor must not undo it.
    desired_running: std::sync::atomic::AtomicBool,
}

impl AppRuntime {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner::default()),
            metrics: Mutex::new(crate::metrics::MetricsTracker::default()),
            desired_running: std::sync::atomic::AtomicBool::new(false),
        }
    }

    pub fn set_desired_running(&self, running: bool) {
        self.desired_running
            .store(running, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn desired_running(&self) -> bool {
        self.desired_running
            .load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn shutdown(&self) {
        self.set_desired_running(false);
        if let Ok(mut g) = self.inner.lock() {
            kill(&mut g.daemon);
            kill(&mut g.client);
            kill(&mut g.agent);
        }
    }
}

pub fn data_dir(app: &AppHandle) -> anyhow::Result<PathBuf> {
    let dir = app.path().app_data_dir().unwrap_or_else(|_| {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        PathBuf::from(home).join(".local/share/nexuskvm")
    });
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn socket_path() -> PathBuf {
    crate::persist::control_socket_path()
}

fn state_path(dir: &Path) -> PathBuf {
    dir.join("state.json")
}

pub(crate) fn load_state(dir: &Path) -> Option<SavedState> {
    let raw = fs::read_to_string(state_path(dir)).ok()?;
    serde_json::from_str(&raw).ok()
}

fn save_state(dir: &Path, state: &SavedState) -> anyhow::Result<()> {
    fs::write(state_path(dir), serde_json::to_string_pretty(state)?)?;
    Ok(())
}

fn set_secret_mode(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
}

fn write_secret(path: &Path, data: impl AsRef<[u8]>) -> anyhow::Result<()> {
    fs::write(path, data)?;
    set_secret_mode(path);
    Ok(())
}

fn password_path(dir: &Path) -> PathBuf {
    dir.join("password")
}
fn cert_path(dir: &Path) -> PathBuf {
    dir.join("certificate.pem")
}
fn key_path(dir: &Path) -> PathBuf {
    dir.join("key.pem")
}
pub(crate) fn daemon_config_path(dir: &Path) -> PathBuf {
    dir.join("daemon.toml")
}
pub(crate) fn client_config_path(dir: &Path) -> PathBuf {
    dir.join("client.toml")
}

fn local_ips() -> Vec<IpAddr> {
    let mut ips = vec![IpAddr::from([127, 0, 0, 1])];
    if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
        if socket.connect("8.8.8.8:80").is_ok() {
            if let Ok(addr) = socket.local_addr() {
                if !ips.contains(&addr.ip()) {
                    ips.push(addr.ip());
                }
            }
        }
    }
    ips
}

fn advertise() -> String {
    local_ips()
        .into_iter()
        .find(|ip| !ip.is_loopback())
        .map(|ip| format!("{ip}:5258"))
        .unwrap_or_else(|| "127.0.0.1:5258".into())
}

fn ensure_password(dir: &Path) -> anyhow::Result<String> {
    let path = password_path(dir);
    if path.exists() {
        return Ok(fs::read_to_string(path)?.trim().to_string());
    }
    let id = uuid::Uuid::new_v4().simple().to_string();
    let pw = id[..12].to_string();
    write_secret(&path, &pw)?;
    Ok(pw)
}

fn generate_certs(dir: &Path) -> anyhow::Result<()> {
    if !(cert_path(dir).exists() && key_path(dir).exists()) {
        write_ca(dir)?;
    }
    if issue_client_cert(dir).is_err() {
        let _ = fs::remove_file(cert_path(dir));
        let _ = fs::remove_file(key_path(dir));
        let _ = fs::remove_file(client_cert_path(dir));
        let _ = fs::remove_file(client_key_path(dir));
        write_ca(dir)?;
        issue_client_cert(dir)?;
    }
    Ok(())
}

fn write_ca(dir: &Path) -> anyhow::Result<()> {
    let mut cfg = String::from(
        "[req]\nprompt = no\ndefault_bits = 2048\ndistinguished_name = req_distinguished_name\nx509_extensions = v3_ca\n[req_distinguished_name]\ncommonName = nexuskvm\n[v3_ca]\nbasicConstraints = critical,CA:TRUE\nkeyUsage = critical, digitalSignature, keyEncipherment, keyCertSign\nsubjectAltName = @alt_names\n[alt_names]\nDNS.1 = localhost\n",
    );
    for (i, ip) in local_ips().iter().enumerate() {
        cfg.push_str(&format!("IP.{} = {}\n", i + 1, ip));
    }
    let cfg_path = dir.join("openssl.cnf");
    fs::write(&cfg_path, cfg)?;
    let status = Command::new("openssl")
        .args([
            "req", "-sha256", "-x509", "-nodes", "-newkey", "rsa:2048", "-keyout",
        ])
        .arg(key_path(dir))
        .arg("-out")
        .arg(cert_path(dir))
        .arg("-config")
        .arg(&cfg_path)
        .arg("-days")
        .arg("3650")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|_| anyhow::anyhow!("openssl is not installed"))?;
    if !status.success() {
        anyhow::bail!("openssl failed to generate the certificate");
    }
    set_secret_mode(&key_path(dir));
    Ok(())
}

fn client_cert_path(dir: &Path) -> PathBuf {
    dir.join("client-cert.pem")
}
fn client_key_path(dir: &Path) -> PathBuf {
    dir.join("client-key.pem")
}

fn issue_client_cert(dir: &Path) -> anyhow::Result<()> {
    if client_cert_path(dir).exists() && client_key_path(dir).exists() {
        return Ok(());
    }
    let csr = dir.join("client.csr");
    let status = Command::new("openssl")
        .args(["req", "-new", "-nodes", "-newkey", "rsa:2048", "-keyout"])
        .arg(client_key_path(dir))
        .arg("-out")
        .arg(&csr)
        .args(["-subj", "/CN=nexuskvm-client"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if !status.success() {
        anyhow::bail!("openssl failed to create client CSR");
    }
    set_secret_mode(&client_key_path(dir));
    let status = Command::new("openssl")
        .args(["x509", "-req", "-in"])
        .arg(&csr)
        .arg("-CA")
        .arg(cert_path(dir))
        .arg("-CAkey")
        .arg(key_path(dir))
        .arg("-CAcreateserial")
        .arg("-out")
        .arg(client_cert_path(dir))
        .args(["-days", "3650", "-sha256"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if !status.success() {
        anyhow::bail!("openssl failed to sign client certificate (re-pair as host)");
    }
    Ok(())
}

fn write_daemon_toml(dir: &Path, password: &str, socket: &Path) -> anyhow::Result<()> {
    let body = format!(
        "socket = \"{}\"\nlisten = \"{LISTEN}\"\nswitch-keys = [\"left-alt\", \"left-ctrl\"]\npropagate-switch-keys = false\ncertificate = \"{}\"\nkey = \"{}\"\npassword = \"{}\"\n",
        socket.display(),
        cert_path(dir).display(),
        key_path(dir).display(),
        password
    );
    fs::write(daemon_config_path(dir), body)?;
    set_secret_mode(&daemon_config_path(dir));
    Ok(())
}

fn write_client_toml(dir: &Path, server: &str, password: &str) -> anyhow::Result<()> {
    let body = format!(
        "server = \"{server}\"\ncertificate = \"{}\"\nclient-certificate = \"{}\"\nclient-key = \"{}\"\npassword = \"{}\"\n",
        cert_path(dir).display(),
        client_cert_path(dir).display(),
        client_key_path(dir).display(),
        password
    );
    fs::write(client_config_path(dir), body)?;
    set_secret_mode(&client_config_path(dir));
    Ok(())
}

fn push_workspace_bins(candidates: &mut Vec<PathBuf>, root: &Path, name: &str) {
    candidates.push(root.join("../target/debug").join(name));
    candidates.push(root.join("../target/release").join(name));
    candidates.push(root.join("../rkvm-master/target/debug").join(name));
    candidates.push(root.join("../rkvm-master/target/release").join(name));
}

fn bin_help_has(path: &Path, flag: &str) -> bool {
    Command::new(path)
        .arg("--help")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()
        .is_some_and(|o| String::from_utf8_lossy(&o.stdout).contains(flag))
}

fn push_named(candidates: &mut Vec<PathBuf>, dir: &Path, name: &str) {
    candidates.push(dir.join(name));
    let triple = env!("TARGET_TRIPLE");
    if !triple.is_empty() {
        candidates.push(dir.join(format!("{name}-{triple}")));
    }
}

pub(crate) fn find_bin(app: &AppHandle, name: &str) -> Option<PathBuf> {
    // Probing runs `--help` on candidates and scans several directories: cache
    // the result for the process lifetime (paths do not change at runtime).
    static BIN_CACHE: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<String, Option<PathBuf>>>,
    > = std::sync::OnceLock::new();
    let cache = BIN_CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    if let Ok(guard) = cache.lock() {
        if let Some(hit) = guard.get(name) {
            return hit.clone();
        }
    }
    let resolved = find_bin_uncached(app, name);
    if let Ok(mut guard) = cache.lock() {
        guard.insert(name.to_string(), resolved.clone());
    }
    resolved
}

fn find_bin_uncached(app: &AppHandle, name: &str) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(dir) = app.path().resource_dir() {
        push_named(&mut candidates, &dir, name);
        push_named(&mut candidates, &dir.join("binaries"), name);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            push_named(&mut candidates, dir, name);
            // cargo tauri: src-tauri/target/debug/<ui> → workspace target/debug/<bin>
            candidates.push(dir.join("../../../target/debug").join(name));
            candidates.push(dir.join("../../../target/release").join(name));
        }
    }
    // CARGO_MANIFEST_DIR exists at compile time for src-tauri, not as a runtime env var.
    if let Some(manifest) = option_env!("CARGO_MANIFEST_DIR") {
        push_workspace_bins(&mut candidates, Path::new(manifest), name);
    }
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        push_workspace_bins(&mut candidates, Path::new(&manifest), name);
    }
    if let Ok(path) = std::env::var("PATH") {
        for dir in path.split(':') {
            candidates.push(PathBuf::from(dir).join(name));
        }
    }
    let mut found: Vec<PathBuf> = candidates.into_iter().filter(|p| p.is_file()).collect();
    found.sort_by_key(|p| {
        std::cmp::Reverse(
            p.metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
        )
    });
    if name == "nexus-kvmd" {
        found.into_iter().find(|p| bin_help_has(p, "--config"))
    } else {
        found.into_iter().next()
    }
}

fn service_log_path(dir: &Path, role: Option<Role>) -> Option<PathBuf> {
    match role {
        Some(Role::Host) => Some(logs_dir(dir).join("nexus-kvmd.log")),
        Some(Role::Client) => Some(logs_dir(dir).join("rkvm-client.log")),
        None => None,
    }
}

fn fail_child(dir: &Path, service: &str, g: &mut Inner) -> Option<String> {
    let log_path = logs_dir(dir).join(format!("{service}.log"));
    let tail = read_log_tail(&log_path, 12);
    let msg = if tail.is_empty() {
        format!("{service} stopped. See {log_path:?}")
    } else {
        format!("{service} stopped:\n{tail}")
    };
    ui_log(dir, &format!("WARN {service} exited: {msg}"));
    g.last_error = Some(msg.clone());
    Some(msg)
}

fn kill(child: &mut Option<Child>) {
    if let Some(mut c) = child.take() {
        let _ = c.kill();
        let _ = c.wait();
    }
}

fn child_running(child: &mut Option<Child>) -> bool {
    let Some(c) = child.as_mut() else {
        return false;
    };
    matches!(c.try_wait(), Ok(None))
}

fn in_input_group() -> bool {
    let Ok(out) = Command::new("id").arg("-nG").output() else {
        return true;
    };
    String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .any(|g| g == "input")
}

fn uinput_accessible() -> bool {
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/uinput")
        .is_ok()
}

fn logs_dir(dir: &Path) -> PathBuf {
    dir.join("logs")
}

/// Per-file cap. A rotated `.log.1` file is kept next to the live log.
const LOG_MAX_BYTES: u64 = 8 * 1024 * 1024;
/// Total cap for the logs directory; older rotated files are pruned.
const LOG_DIR_MAX_BYTES: u64 = 64 * 1024 * 1024;

fn rotated_path(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(".1");
    PathBuf::from(s)
}

fn rotate_if_needed(path: &Path) {
    let Ok(meta) = fs::metadata(path) else {
        return;
    };
    if meta.len() <= LOG_MAX_BYTES {
        return;
    }
    let rotated = rotated_path(path);
    let _ = fs::remove_file(&rotated);
    let _ = fs::rename(path, &rotated);
}

fn prune_logs_dir(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut rotated: Vec<(std::time::SystemTime, u64, PathBuf)> = Vec::new();
    let mut total = 0u64;
    for e in entries.flatten() {
        let Ok(meta) = e.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        total = total.saturating_add(meta.len());
        let name = e.file_name();
        let name = name.to_string_lossy();
        if name.ends_with(".log.1") {
            rotated.push((
                meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH),
                meta.len(),
                e.path(),
            ));
        }
    }
    if total <= LOG_DIR_MAX_BYTES {
        return;
    }
    rotated.sort_by_key(|(t, _, _)| *t);
    for (_, len, path) in rotated {
        let _ = fs::remove_file(&path);
        total = total.saturating_sub(len);
        if total <= LOG_DIR_MAX_BYTES {
            break;
        }
    }
}

fn open_log(path: &Path) -> Option<fs::File> {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .ok()
}

pub(crate) fn ui_log(dir: &Path, msg: &str) {
    let path = logs_dir(dir).join("nexuskvm-ui.log");
    rotate_if_needed(&path);
    use std::io::Write;
    if let Some(mut f) = open_log(&path) {
        let _ = writeln!(f, "{msg}");
    }
}

/// Pump a child pipe into a size-capped log file, rotating when full.
fn spawn_log_pump(reader: impl std::io::Read + Send + 'static, path: PathBuf) {
    std::thread::spawn(move || {
        use std::io::{BufRead, BufReader, Write};
        let mut reader = BufReader::new(reader);
        let mut file = open_log(&path);
        let mut written = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match reader.read_until(b'\n', &mut buf) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    if written + buf.len() as u64 > LOG_MAX_BYTES {
                        let rotated = rotated_path(&path);
                        let _ = fs::remove_file(&rotated);
                        let _ = fs::rename(&path, &rotated);
                        file = open_log(&path);
                        written = 0;
                    }
                    if let Some(f) = file.as_mut() {
                        if f.write_all(&buf).is_ok() {
                            written = written.saturating_add(buf.len() as u64);
                        }
                    }
                }
            }
        }
        prune_logs_dir(path.parent().unwrap_or(Path::new(".")));
    });
}

fn strip_ansi(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
            i += 2;
            while i < bytes.len() {
                let c = bytes[i];
                i += 1;
                if (b'@'..=b'~').contains(&c) {
                    break;
                }
            }
            continue;
        }
        // leftover CSI without ESC from some terminals / copy-paste
        if bytes[i] == b'[' && i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit() {
            let start = i;
            i += 1;
            while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b';') {
                i += 1;
            }
            if i < bytes.len() && (bytes[i] == b'm' || bytes[i] == b'K') {
                i += 1;
                continue;
            }
            i = start;
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

fn read_log_tail(path: &Path, max_lines: usize) -> String {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(mut f) = fs::File::open(path) else {
        return String::new();
    };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    let window = 256 * 1024u64;
    let start = len.saturating_sub(window);
    if f.seek(SeekFrom::Start(start)).is_err() {
        return String::new();
    }
    let mut raw = String::new();
    let _ = f.take(window).read_to_string(&mut raw);
    let cleaned = strip_ansi(&raw);
    let lines: Vec<&str> = cleaned
        .lines()
        .filter(|l| {
            let t = l.trim();
            !t.is_empty() && !t.contains("Received ping") && !t.contains("Sent pong")
        })
        .collect();
    if lines.len() <= max_lines {
        return lines.join("\n");
    }
    lines[lines.len() - max_lines..].join("\n")
}

fn read_agent_status(dir: &Path) -> Option<AgentStatusFile> {
    let raw = fs::read_to_string(layout_store::agent_status_path(dir)).ok()?;
    serde_json::from_str(&raw).ok()
}

fn ensure_default_layout(dir: &Path) -> anyhow::Result<()> {
    if !layout_store::layout_path(dir).is_file() {
        layout_store::save(dir, &LayoutFile::default_right(None))?;
    }
    Ok(())
}

fn spawn_agent(
    app: &AppHandle,
    dir: &Path,
    role: Role,
    server: Option<&str>,
) -> anyhow::Result<Child> {
    let agent_bin = find_bin(app, "nexus-agent").ok_or_else(|| {
        anyhow::anyhow!("nexus-agent not found; build it with: cargo build -p nexus-agent")
    })?;
    let sock = socket_path().to_string_lossy().to_string();
    let data = dir.to_string_lossy().to_string();
    let role_s = match role {
        Role::Host => "host",
        Role::Client => "client",
    };
    let mut cmd_args = vec![
        "--socket".into(),
        sock,
        "--data-dir".into(),
        data,
        "--role".into(),
        role_s.into(),
    ];
    let password = fs::read_to_string(password_path(dir))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if let Some(s) = server {
        cmd_args.push("--server".into());
        cmd_args.push(s.to_string());
    }
    let arg_refs: Vec<&str> = cmd_args.iter().map(String::as_str).collect();
    spawn_logged(
        dir,
        "nexus-agent",
        &agent_bin,
        &arg_refs,
        "nexus_agent=debug,nexus=debug",
        password,
    )
}

fn spawn_logged(
    dir: &Path,
    service: &str,
    bin: &Path,
    args: &[&str],
    rust_log: &str,
    secret_env: Option<String>,
) -> anyhow::Result<Child> {
    let log_root = logs_dir(dir);
    fs::create_dir_all(&log_root)?;
    prune_logs_dir(&log_root);
    let log_path = log_root.join(format!("{service}.log"));
    rotate_if_needed(&log_path);
    ui_log(
        dir,
        &format!(
            "spawn {service}: {} args={args:?} log={}",
            bin.display(),
            log_path.display()
        ),
    );
    let mut cmd = Command::new(bin);
    cmd.args(args)
        .env("RUST_LOG", rust_log)
        .env("NO_COLOR", "1")
        .env("RUST_LOG_STYLE", "never")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(pw) = secret_env {
        cmd.env("NEXUSKVM_PASSWORD", pw);
    }
    let mut child = cmd
        .spawn()
        .map_err(|e| anyhow::anyhow!("failed to start {}: {e}", bin.display()))?;
    if let Some(out) = child.stdout.take() {
        spawn_log_pump(out, log_path.clone());
    }
    if let Some(err) = child.stderr.take() {
        spawn_log_pump(err, log_path);
    }
    Ok(child)
}

/// Keeps the role processes alive and resyncs them when they die.
///
/// `rkvm-client` exits on any connection error and the daemon/agent can be
/// killed by an upgrade or a crash. Without a supervisor the machine stays
/// dead until the app is reopened by hand, and the host keeps routing input to
/// a peer that no longer receives it.
pub fn spawn_supervisor(app: AppHandle, rt: Arc<AppRuntime>) {
    tauri::async_runtime::spawn(async move {
        let mut fails: std::collections::HashMap<&'static str, u32> =
            std::collections::HashMap::new();
        let mut last: std::collections::HashMap<&'static str, std::time::Instant> =
            std::collections::HashMap::new();
        loop {
            sleep(Duration::from_secs(2)).await;
            // An explicit Stop must stick: never resurrect what the user stopped.
            if !rt.desired_running() {
                continue;
            }
            let Ok(dir) = data_dir(&app) else { continue };
            let Some(state) = load_state(&dir) else {
                continue;
            };
            supervise_once(&app, &rt, &dir, &state, &mut fails, &mut last).await;
        }
    });
}

fn should_attempt(
    key: &'static str,
    fails: &mut std::collections::HashMap<&'static str, u32>,
    last: &mut std::collections::HashMap<&'static str, std::time::Instant>,
) -> bool {
    let now = std::time::Instant::now();
    let attempts = fails.get(key).copied().unwrap_or(0).min(5);
    let backoff = Duration::from_secs(2u64.saturating_pow(attempts).min(30));
    if let Some(t) = last.get(key) {
        if now.duration_since(*t) < backoff {
            return false;
        }
    }
    last.insert(key, now);
    *fails.entry(key).or_insert(0) += 1;
    true
}

async fn supervise_once(
    app: &AppHandle,
    rt: &AppRuntime,
    dir: &Path,
    state: &SavedState,
    fails: &mut std::collections::HashMap<&'static str, u32>,
    last: &mut std::collections::HashMap<&'static str, std::time::Instant>,
) {
    let role = state.role;
    let role_key: &'static str = match role {
        Role::Host => "nexus-kvmd",
        Role::Client => "rkvm-client",
    };
    if !crate::persist::boot_service_active(role) {
        let alive = match rt.inner.lock() {
            Ok(mut g) => match role {
                Role::Host => child_running(&mut g.daemon),
                Role::Client => child_running(&mut g.client),
            },
            Err(_) => true,
        };
        let reachable = if role == Role::Host {
            alive || socket_alive().await
        } else {
            alive
        };
        if reachable {
            fails.insert(role_key, 0);
        } else if should_attempt(role_key, fails, last) {
            let bin = match role {
                Role::Host => find_bin(app, "nexus-kvmd"),
                Role::Client => find_bin(app, "rkvm-client"),
            };
            let cfg = match role {
                Role::Host => daemon_config_path(dir),
                Role::Client => client_config_path(dir),
            };
            let Some(bin) = bin else {
                ui_log(dir, &format!("supervisor: {role_key} binary not found"));
                return;
            };
            if !cfg.is_file() {
                ui_log(
                    dir,
                    &format!("supervisor: {role_key} config missing ({})", cfg.display()),
                );
                return;
            }
            let cfg_s = cfg.to_string_lossy().to_string();
            let rust_log = match role {
                Role::Host => "nexus=info,rkvm_server=info,rkvm_input=info",
                Role::Client => "rkvm_client=info,rkvm_input=info",
            };
            // nexus-kvmd only accepts `--config <path>`; rkvm-client takes a
            // positional config path. Passing the bare path to the daemon made
            // every supervised restart die with "unexpected argument".
            let args: Vec<String> = match role {
                Role::Host => vec!["--config".into(), cfg_s.clone()],
                Role::Client => vec![cfg_s.clone()],
            };
            let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
            match spawn_logged(dir, role_key, &bin, &arg_refs, rust_log, None) {
                Ok(child) => {
                    if let Ok(mut g) = rt.inner.lock() {
                        match role {
                            Role::Host => g.daemon = Some(child),
                            Role::Client => g.client = Some(child),
                        }
                        g.last_error = None;
                    }
                    // Do NOT reset the backoff here: the child may die in the
                    // next instant. It is reset once the process is observed
                    // alive (see the `reachable` branch).
                    ui_log(dir, &format!("supervisor: restarted {role_key}"));
                }
                Err(e) => {
                    ui_log(dir, &format!("supervisor: {role_key} restart failed: {e}"));
                }
            }
        }
    }

    // Session agent (clipboard + SwitchLocal listener) for both roles.
    let agent_alive = match rt.inner.lock() {
        Ok(mut g) => child_running(&mut g.agent),
        Err(_) => true,
    };
    if agent_alive {
        fails.insert("nexus-agent", 0);
    } else if should_attempt("nexus-agent", fails, last) {
        match spawn_agent(app, dir, role, state.server.as_deref()) {
            Ok(child) => {
                if let Ok(mut g) = rt.inner.lock() {
                    kill(&mut g.agent);
                    g.agent = Some(child);
                }
                // Backoff resets only when the agent is observed alive.
                ui_log(dir, "supervisor: restarted nexus-agent");
            }
            Err(e) => {
                ui_log(dir, &format!("supervisor: nexus-agent restart failed: {e}"));
            }
        }
    }
}

fn child_failure(dir: &Path, service: &str) -> String {
    let log_path = logs_dir(dir).join(format!("{service}.log"));
    let tail = read_log_tail(&log_path, 16);
    if tail.is_empty() {
        format!(
            "{service} exited on startup. Check {}/logs/{service}.log",
            dir.display()
        )
    } else {
        format!("{service} exited on startup. Log:\n{tail}")
    }
}

fn host_preflight() -> anyhow::Result<()> {
    session_preflight()
}

fn client_preflight() -> anyhow::Result<()> {
    session_preflight()
}

fn session_preflight() -> anyhow::Result<()> {
    if !in_input_group() {
        anyhow::bail!(
            "Your user still lacks permission on /dev/uinput. \
             Log out and back in (or reboot) after installing NexusKVM."
        );
    }
    if !uinput_accessible() {
        anyhow::bail!(
            "Cannot open /dev/uinput. Restart your session; if it still fails, \
             check that the package installed the udev rules (input group)."
        );
    }
    Ok(())
}

async fn socket_alive() -> bool {
    control_client().send(ControlCommand::Status).await.is_ok()
}

async fn daemon_status_opt() -> Option<AppStatus> {
    let r = control_client().send(ControlCommand::Status).await.ok()?;
    if r.ok {
        r.status
    } else {
        None
    }
}

pub async fn snapshot(app: &AppHandle, rt: &AppRuntime) -> RuntimeSnapshot {
    let dir = data_dir(app).ok();
    let state = dir.as_ref().and_then(|d| load_state(d));
    let role = state.as_ref().map(|s| s.role);
    let remote_server = state.as_ref().and_then(|s| s.server.clone());
    let has_password = dir
        .as_ref()
        .and_then(|d| fs::read_to_string(password_path(d)).ok())
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    let (daemon_alive, client_alive, error, child_pid) = match rt.inner.lock() {
        Ok(mut g) => {
            let d_alive = child_running(&mut g.daemon);
            let c_alive = child_running(&mut g.client);
            let _a_alive = child_running(&mut g.agent);
            if !d_alive {
                if g.daemon.is_some() {
                    if let Some(d) = dir.as_ref() {
                        fail_child(d, "nexus-kvmd", &mut g);
                    }
                }
                g.daemon = None;
            }
            if !c_alive {
                if g.client.is_some() {
                    if let Some(d) = dir.as_ref() {
                        fail_child(d, "rkvm-client", &mut g);
                    }
                }
                g.client = None;
            }
            if !child_running(&mut g.agent) {
                g.agent = None;
            }
            let pid = match role {
                Some(Role::Host) => g.daemon.as_ref().map(|c| c.id()),
                Some(Role::Client) => g
                    .client
                    .as_ref()
                    .map(|c| c.id())
                    .or_else(|| g.agent.as_ref().map(|c| c.id())),
                None => None,
            };
            (d_alive, c_alive, g.last_error.clone(), pid)
        }
        Err(_) => (false, false, None, None),
    };
    let sock = if matches!(role, Some(Role::Host)) {
        socket_alive().await
    } else {
        false
    };
    let service_ok = match role {
        Some(Role::Host) => sock || daemon_alive || crate::persist::boot_service_active(Role::Host),
        Some(Role::Client) => client_alive || crate::persist::boot_service_active(Role::Client),
        None => false,
    };
    let running = service_ok;
    let log_dir = dir.as_ref().map(|d| logs_dir(d).display().to_string());
    let service_log = dir
        .as_ref()
        .and_then(|d| service_log_path(d, role).map(|p| read_log_tail(&p, 14)));
    let agent_st = dir.as_ref().and_then(|d| read_agent_status(d));
    let layout_side = dir.as_ref().and_then(|d| {
        layout_store::load_or_default(d)
            .ok()
            .map(|f| match f.peer_side {
                PeerSide::Left => "left".into(),
                PeerSide::Right => "right".into(),
                PeerSide::Top => "top".into(),
                PeerSide::Bottom => "bottom".into(),
            })
    });
    let daemon = if matches!(role, Some(Role::Host)) {
        daemon_status_opt().await
    } else {
        None
    };
    let portal_available = daemon
        .as_ref()
        .map(|d| d.portal_available)
        .or_else(|| agent_st.as_ref().map(|a| a.portal_available))
        .unwrap_or(false);
    let portal_error = agent_st.as_ref().and_then(|a| a.portal_error.clone());
    let clipboard_ok = agent_st.as_ref().map(|a| a.clipboard_ok).unwrap_or(false);
    // Real process metrics: prefer our own child handle, else find the systemd
    // unit's process by binary name.
    let (_svc_bin, svc_name) = match role {
        Some(Role::Host) => ("nexus-kvmd", "nexus-kvmd"),
        Some(Role::Client) => ("rkvm-client", "rkvm-client"),
        None => ("", ""),
    };
    let metrics_pid = child_pid.or_else(|| match role {
        Some(Role::Host) => crate::metrics::find_service_pid("nexus-kvmd")
            .or_else(|| crate::metrics::find_service_pid("rkvm-server")),
        Some(Role::Client) => crate::metrics::find_service_pid("rkvm-client"),
        None => None,
    });
    let track = service_ok || metrics_pid.is_some();
    let metrics = match rt.metrics.lock() {
        Ok(mut t) => {
            if track {
                t.sample(
                    metrics_pid,
                    if svc_name.is_empty() {
                        None
                    } else {
                        Some(svc_name)
                    },
                )
            } else {
                t.sample(None, None)
            }
        }
        Err(_) => crate::metrics::ServiceMetrics::default(),
    };
    RuntimeSnapshot {
        role,
        running,
        socket_ok: sock,
        service_ok,
        listen: LISTEN.into(),
        advertise: advertise(),
        remote_server,
        password: String::new(),
        has_password,
        error,
        needs_logout: !in_input_group(),
        log_dir,
        service_log: service_log.filter(|s| !s.is_empty()),
        daemon,
        binary_host: find_bin(app, "nexus-kvmd").map(|p| p.display().to_string()),
        binary_client: find_bin(app, "rkvm-client").map(|p| p.display().to_string()),
        peer_side: layout_side.or_else(|| agent_st.map(|a| a.peer_side)),
        portal_available,
        portal_error,
        clipboard_ok,
        metrics,
    }
}

pub async fn setup_host(app: &AppHandle, rt: &AppRuntime) -> anyhow::Result<RuntimeSnapshot> {
    let dir = data_dir(app)?;
    let password = ensure_password(&dir)?;
    generate_certs(&dir)?;
    write_daemon_toml(&dir, &password, &crate::persist::control_socket_path())?;
    ensure_default_layout(&dir)?;
    save_state(
        &dir,
        &SavedState {
            role: Role::Host,
            server: None,
        },
    )?;
    start(app, rt).await?;
    let _ = crate::persist::install_persistence(app, &dir);
    detach_if_boot_owned(rt, Role::Host);
    Ok(snapshot(app, rt).await)
}

pub async fn setup_client(
    app: &AppHandle,
    rt: &AppRuntime,
    invite: Invite,
) -> anyhow::Result<RuntimeSnapshot> {
    let dir = data_dir(app)?;
    write_secret(&password_path(&dir), invite.password.trim())?;
    fs::write(cert_path(&dir), invite.certificate.trim_start())?;
    if !invite.client_certificate.trim().is_empty() {
        fs::write(
            client_cert_path(&dir),
            invite.client_certificate.trim_start(),
        )?;
    }
    if !invite.client_key.trim().is_empty() {
        fs::write(client_key_path(&dir), invite.client_key.trim_start())?;
        set_secret_mode(&client_key_path(&dir));
    }
    write_client_toml(&dir, invite.server.trim(), invite.password.trim())?;
    save_state(
        &dir,
        &SavedState {
            role: Role::Client,
            server: Some(invite.server.trim().into()),
        },
    )?;
    start(app, rt).await?;
    let _ = crate::persist::install_persistence(app, &dir);
    detach_if_boot_owned(rt, Role::Client);
    Ok(snapshot(app, rt).await)
}

fn detach_if_boot_owned(rt: &AppRuntime, role: Role) {
    if !crate::persist::boot_service_active(role) {
        return;
    }
    let mut reaped: Vec<std::process::Child> = Vec::new();
    if let Ok(mut g) = rt.inner.lock() {
        // systemd owns the role binary; drop child handles without killing.
        if let Some(child) = g.daemon.take() {
            reaped.push(child);
        }
        if let Some(child) = g.client.take() {
            reaped.push(child);
        }
    }
    // Reap asynchronously so a killed session copy cannot linger as a zombie.
    for mut child in reaped {
        std::thread::spawn(move || {
            let _ = child.wait();
        });
    }
}

pub fn reset_setup(app: &AppHandle, rt: &AppRuntime) -> anyhow::Result<()> {
    rt.shutdown();
    crate::persist::clear_persistence(app);
    if let Ok(dir) = data_dir(app) {
        let _ = fs::remove_file(state_path(&dir));
    }
    Ok(())
}

pub fn open_logs(app: &AppHandle) -> anyhow::Result<String> {
    let dir = data_dir(app)?;
    let logs = logs_dir(&dir);
    fs::create_dir_all(&logs)?;
    ui_log(&dir, "open_logs");
    #[cfg(target_os = "linux")]
    {
        let status = Command::new("xdg-open").arg(&logs).status();
        if status.map(|s| s.success()).unwrap_or(false) {
            return Ok(logs.display().to_string());
        }
    }
    Ok(logs.display().to_string())
}

pub async fn start(app: &AppHandle, rt: &AppRuntime) -> anyhow::Result<()> {
    let dir = data_dir(app)?;
    rt.set_desired_running(true);
    ui_log(&dir, "start_runtime");
    let state =
        load_state(&dir).ok_or_else(|| anyhow::anyhow!("this machine is not configured yet"))?;
    match state.role {
        Role::Host => {
            host_preflight()?;
            ensure_default_layout(&dir)?;
            let boot = crate::persist::boot_service_active(Role::Host);
            if boot {
                ui_log(&dir, "host boot service already active");
                if let Ok(mut g) = rt.inner.lock() {
                    g.last_error = None;
                    // systemd owns the daemon; drop any stale child handle.
                    let _ = g.daemon.take();
                }
            } else if !socket_alive().await {
                let bin = find_bin(app, "nexus-kvmd").ok_or_else(|| {
                    anyhow::anyhow!(
                        "current nexus-kvmd not found (with --config). Build with: cargo build -p nexus-daemon --bin nexus-kvmd"
                    )
                })?;
                let cfg = daemon_config_path(&dir);
                let cfg_s = cfg.to_string_lossy().to_string();
                {
                    let mut g = rt
                        .inner
                        .lock()
                        .map_err(|_| anyhow::anyhow!("runtime busy"))?;
                    g.last_error = None;
                    kill(&mut g.daemon);
                    g.daemon = Some(spawn_logged(
                        &dir,
                        "nexus-kvmd",
                        &bin,
                        &["--config", &cfg_s],
                        "nexus=info,rkvm_server=info,rkvm_input=info",
                        None,
                    )?);
                }
                sleep(Duration::from_millis(800)).await;
                {
                    let mut g = rt
                        .inner
                        .lock()
                        .map_err(|_| anyhow::anyhow!("runtime busy"))?;
                    if !child_running(&mut g.daemon) {
                        let msg = child_failure(&dir, "nexus-kvmd");
                        g.last_error = Some(msg.clone());
                        ui_log(&dir, &format!("ERROR nexus-kvmd: {msg}"));
                        anyhow::bail!(msg);
                    }
                }
            } else {
                ui_log(&dir, "daemon socket already alive");
                if let Ok(mut g) = rt.inner.lock() {
                    g.last_error = None;
                }
            }
            {
                let agent = spawn_agent(app, &dir, Role::Host, None)?;
                let mut g = rt
                    .inner
                    .lock()
                    .map_err(|_| anyhow::anyhow!("runtime busy"))?;
                kill(&mut g.agent);
                g.agent = Some(agent);
            }
            // Give the agent / socket a moment when talking to the system unit.
            if boot && !socket_alive().await {
                sleep(Duration::from_millis(500)).await;
            }
            if !socket_alive().await {
                let mut g = rt
                    .inner
                    .lock()
                    .map_err(|_| anyhow::anyhow!("runtime busy"))?;
                let msg = if boot {
                    format!(
                        "nexus-kvmd boot service is running but the control socket \
                         ({}) is not reachable. Are you in the input group? Log out \
                         and back in after install.",
                        socket_path().display()
                    )
                } else {
                    child_failure(&dir, "nexus-kvmd")
                };
                g.last_error = Some(msg.clone());
                ui_log(&dir, &format!("daemon socket missing: {msg}"));
                anyhow::bail!(msg);
            }
            ui_log(&dir, "host runtime started");
        }
        Role::Client => {
            client_preflight()?;
            let boot = crate::persist::boot_service_active(Role::Client);
            if boot {
                ui_log(&dir, "client boot service already active");
            } else {
                let bin = find_bin(app, "rkvm-client").ok_or_else(|| {
                    anyhow::anyhow!(
                        "rkvm-client not found. Build it with: cargo build -p rkvm-client --manifest-path rkvm-master/Cargo.toml"
                    )
                })?;
                let cfg = client_config_path(&dir);
                if !cfg.is_file() {
                    anyhow::bail!(
                        "missing client.toml; reconnect using the host machine's invite code"
                    );
                }
                let cfg_s = cfg.to_string_lossy().to_string();
                ui_log(&dir, &format!("client config: {cfg_s}"));
                {
                    let mut g = rt
                        .inner
                        .lock()
                        .map_err(|_| anyhow::anyhow!("runtime busy"))?;
                    g.last_error = None;
                    kill(&mut g.client);
                    g.client = Some(spawn_logged(
                        &dir,
                        "rkvm-client",
                        &bin,
                        &[&cfg_s],
                        "rkvm_client=info,rkvm_input=info",
                        None,
                    )?);
                }
                sleep(Duration::from_millis(1500)).await;
                {
                    let mut g = rt
                        .inner
                        .lock()
                        .map_err(|_| anyhow::anyhow!("runtime busy"))?;
                    if !child_running(&mut g.client) {
                        let msg = child_failure(&dir, "rkvm-client");
                        g.last_error = Some(msg.clone());
                        ui_log(&dir, &format!("ERROR rkvm-client: {msg}"));
                        anyhow::bail!(msg);
                    }
                }
            }
            let server = state.server.clone().or_else(|| {
                fs::read_to_string(client_config_path(&dir))
                    .ok()
                    .and_then(|t| {
                        t.lines().find_map(|l| {
                            let rest = l.trim().strip_prefix("server")?;
                            let rest = rest.trim().strip_prefix('=')?.trim();
                            Some(rest.trim_matches('"').to_string())
                        })
                    })
            });
            // Return layout: host on the left by default.
            if !layout_store::layout_path(&dir).is_file() {
                let f = LayoutFile::default_right(None).with_side(PeerSide::Left);
                layout_store::save(&dir, &f)?;
            }
            {
                let agent = spawn_agent(app, &dir, Role::Client, server.as_deref())?;
                let mut g = rt
                    .inner
                    .lock()
                    .map_err(|_| anyhow::anyhow!("runtime busy"))?;
                kill(&mut g.agent);
                g.agent = Some(agent);
            }
            ui_log(&dir, "client runtime started");
        }
    }
    Ok(())
}

pub fn invite(app: &AppHandle) -> anyhow::Result<Invite> {
    let dir = data_dir(app)?;
    let password = fs::read_to_string(password_path(&dir))
        .map_err(|_| anyhow::anyhow!("no password; configure this machine as host"))?;
    let certificate =
        fs::read_to_string(cert_path(&dir)).map_err(|_| anyhow::anyhow!("no certificate yet"))?;
    let client_certificate = fs::read_to_string(client_cert_path(&dir)).unwrap_or_default();
    let client_key = fs::read_to_string(client_key_path(&dir)).unwrap_or_default();
    Ok(Invite {
        server: advertise(),
        password: password.trim().into(),
        certificate,
        client_certificate,
        client_key,
    })
}

pub fn set_peer_side(app: &AppHandle, side: &str) -> anyhow::Result<LayoutFile> {
    let dir = data_dir(app)?;
    let peer_side = match side {
        "left" => PeerSide::Left,
        "right" => PeerSide::Right,
        "top" => PeerSide::Top,
        "bottom" => PeerSide::Bottom,
        _ => anyhow::bail!("invalid side: {side} (left|right|top|bottom)"),
    };
    let mut file = layout_store::load_or_default(&dir)?;
    file = file.with_side(peer_side);
    layout_store::save(&dir, &file)?;
    Ok(file)
}

pub fn get_layout(app: &AppHandle) -> anyhow::Result<LayoutFile> {
    let dir = data_dir(app)?;
    layout_store::load_or_default(&dir)
}

pub fn control_client() -> DaemonClient {
    let token = dirs_password();
    DaemonClient {
        socket: socket_path().to_string_lossy().into(),
        token,
    }
}

fn dirs_password() -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    let candidates = [
        PathBuf::from(&home).join(".local/share/io.nexuskvm.app/password"),
        PathBuf::from(&home).join(".local/share/nexuskvm/password"),
    ];
    for path in candidates {
        if let Ok(s) = fs::read_to_string(&path) {
            let t = s.trim().to_string();
            if !t.is_empty() {
                return Some(t);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invite_roundtrip() {
        let inv = Invite {
            server: "10.0.0.2:5258".into(),
            password: "abc".into(),
            certificate: "-----BEGIN CERTIFICATE-----\nX\n-----END CERTIFICATE-----\n".into(),
            client_certificate: String::new(),
            client_key: String::new(),
        };
        let s = serde_json::to_string(&inv).unwrap();
        let back: Invite = serde_json::from_str(&s).unwrap();
        assert_eq!(back.server, "10.0.0.2:5258");
    }

    #[test]
    fn strips_ansi_and_csi() {
        let raw = "\u{1b}[31mERROR\u{1b}[0m boom\n[34mDEBUG[0m ping";
        let cleaned = strip_ansi(raw);
        assert!(cleaned.contains("ERROR boom"));
        assert!(!cleaned.contains("31m"));
        assert!(!cleaned.contains("[34m"));
    }

    #[test]
    fn supervisor_backoff_blocks_immediate_retry() {
        let mut fails = std::collections::HashMap::new();
        let mut last = std::collections::HashMap::new();
        assert!(should_attempt("svc", &mut fails, &mut last));
        assert!(!should_attempt("svc", &mut fails, &mut last));
        fails.insert("svc", 0);
        last.insert("svc", std::time::Instant::now() - Duration::from_secs(60));
        assert!(should_attempt("svc", &mut fails, &mut last));
    }

    #[test]
    fn log_rotation_keeps_rotated_copy() {
        let dir = std::env::temp_dir().join(format!("nexus-logrot-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("x.log");
        fs::write(&path, vec![b'a'; (LOG_MAX_BYTES + 1) as usize]).unwrap();
        rotate_if_needed(&path);
        assert!(rotated_path(&path).exists());
        assert!(!path.exists());
        let _ = fs::remove_dir_all(dir);
    }
}
