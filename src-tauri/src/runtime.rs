use nexus_agent::daemon_client::DaemonClient;
use nexus_agent::layout_store::{self, AgentStatusFile};
use nexus_common::*;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    net::{IpAddr, UdpSocket},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::Mutex,
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
    pub password: String,
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
}

impl AppRuntime {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner::default()),
        }
    }

    pub fn shutdown(&self) {
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

fn runtime_dir() -> PathBuf {
    let base = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| {
        format!(
            "/tmp/nexuskvm-{}",
            std::env::var("USER").unwrap_or_else(|_| "user".into())
        )
    });
    let dir = PathBuf::from(base).join("nexuskvm");
    let _ = fs::create_dir_all(&dir);
    dir
}

pub fn socket_path() -> PathBuf {
    runtime_dir().join("control.sock")
}

fn state_path(dir: &Path) -> PathBuf {
    dir.join("state.json")
}

fn load_state(dir: &Path) -> Option<SavedState> {
    let raw = fs::read_to_string(state_path(dir)).ok()?;
    serde_json::from_str(&raw).ok()
}

fn save_state(dir: &Path, state: &SavedState) -> anyhow::Result<()> {
    fs::write(state_path(dir), serde_json::to_string_pretty(state)?)?;
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
fn daemon_config_path(dir: &Path) -> PathBuf {
    dir.join("daemon.toml")
}
fn client_config_path(dir: &Path) -> PathBuf {
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
    fs::write(&path, &pw)?;
    Ok(pw)
}

fn generate_certs(dir: &Path) -> anyhow::Result<()> {
    if cert_path(dir).exists() && key_path(dir).exists() {
        return Ok(());
    }
    let mut cfg = String::from(
        "[req]\nprompt = no\ndefault_bits = 2048\ndistinguished_name = req_distinguished_name\nreq_extensions = req_ext\nx509_extensions = v3_req\n[req_distinguished_name]\ncommonName = nexuskvm\n[req_ext]\nsubjectAltName = @alt_names\n[v3_req]\nsubjectAltName = @alt_names\n[alt_names]\nDNS.1 = localhost\n",
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
    Ok(())
}

fn write_daemon_toml(dir: &Path, password: &str, socket: &Path) -> anyhow::Result<()> {
    let body = format!(
        "socket = \"{}\"\nlisten = \"{LISTEN}\"\nswitch-keys = [\"left-alt\", \"left-ctrl\"]\ncertificate = \"{}\"\nkey = \"{}\"\npassword = \"{}\"\n",
        socket.display(),
        cert_path(dir).display(),
        key_path(dir).display(),
        password
    );
    fs::write(daemon_config_path(dir), body)?;
    Ok(())
}

fn write_client_toml(dir: &Path, server: &str, password: &str) -> anyhow::Result<()> {
    let body = format!(
        "server = \"{server}\"\ncertificate = \"{}\"\npassword = \"{password}\"\n",
        cert_path(dir).display()
    );
    fs::write(client_config_path(dir), body)?;
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

fn find_bin(app: &AppHandle, name: &str) -> Option<PathBuf> {
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
        format!("{service} se detuvo. Ver {log_path:?}")
    } else {
        format!("{service} se detuvo:\n{tail}")
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

fn ui_log(dir: &Path, msg: &str) {
    let path = logs_dir(dir).join("nexuskvm-ui.log");
    let _ = fs::create_dir_all(logs_dir(dir));
    use std::io::Write;
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{msg}");
    }
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
    let Ok(raw) = fs::read_to_string(path) else {
        return String::new();
    };
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
    )
}

fn spawn_logged(
    dir: &Path,
    service: &str,
    bin: &Path,
    args: &[&str],
    rust_log: &str,
) -> anyhow::Result<Child> {
    let log_root = logs_dir(dir);
    fs::create_dir_all(&log_root)?;
    let log_path = log_root.join(format!("{service}.log"));
    ui_log(
        dir,
        &format!(
            "spawn {service}: {} args={args:?} log={}",
            bin.display(),
            log_path.display()
        ),
    );
    let log_file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;
    let log_err = log_file.try_clone()?;
    Command::new(bin)
        .args(args)
        .env("RUST_LOG", rust_log)
        .env("NO_COLOR", "1")
        .env("RUST_LOG_STYLE", "never")
        .stdin(Stdio::null())
        .stdout(Stdio::from(log_file))
        .stderr(Stdio::from(log_err))
        .spawn()
        .map_err(|e| anyhow::anyhow!("failed to start {}: {e}", bin.display()))
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
    let password = dir
        .as_ref()
        .and_then(|d| fs::read_to_string(password_path(d)).ok())
        .unwrap_or_default()
        .trim()
        .to_string();
    let (daemon_alive, client_alive, error) = match rt.inner.lock() {
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
            (d_alive, c_alive, g.last_error.clone())
        }
        Err(_) => (false, false, None),
    };
    let sock = if matches!(role, Some(Role::Host)) {
        socket_alive().await
    } else {
        false
    };
    let service_ok = match role {
        Some(Role::Host) => sock || daemon_alive,
        Some(Role::Client) => client_alive,
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
    let clipboard_ok = agent_st
        .as_ref()
        .map(|a| a.clipboard_ok)
        .unwrap_or(false);
    RuntimeSnapshot {
        role,
        running,
        socket_ok: sock,
        service_ok,
        listen: LISTEN.into(),
        advertise: advertise(),
        remote_server,
        password,
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
    }
}

pub async fn setup_host(app: &AppHandle, rt: &AppRuntime) -> anyhow::Result<RuntimeSnapshot> {
    let dir = data_dir(app)?;
    let password = ensure_password(&dir)?;
    generate_certs(&dir)?;
    write_daemon_toml(&dir, &password, &socket_path())?;
    ensure_default_layout(&dir)?;
    save_state(
        &dir,
        &SavedState {
            role: Role::Host,
            server: None,
        },
    )?;
    start(app, rt).await?;
    Ok(snapshot(app, rt).await)
}

pub async fn setup_client(
    app: &AppHandle,
    rt: &AppRuntime,
    invite: Invite,
) -> anyhow::Result<RuntimeSnapshot> {
    let dir = data_dir(app)?;
    fs::write(password_path(&dir), invite.password.trim())?;
    fs::write(cert_path(&dir), invite.certificate.trim_start())?;
    write_client_toml(&dir, invite.server.trim(), invite.password.trim())?;
    save_state(
        &dir,
        &SavedState {
            role: Role::Client,
            server: Some(invite.server.trim().into()),
        },
    )?;
    start(app, rt).await?;
    Ok(snapshot(app, rt).await)
}

pub fn reset_setup(app: &AppHandle, rt: &AppRuntime) -> anyhow::Result<()> {
    rt.shutdown();
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
    ui_log(&dir, "start_runtime");
    let state =
        load_state(&dir).ok_or_else(|| anyhow::anyhow!("this machine is not configured yet"))?;
    match state.role {
        Role::Host => {
            host_preflight()?;
            ensure_default_layout(&dir)?;
            if !socket_alive().await {
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
            if !socket_alive().await {
                let mut g = rt
                    .inner
                    .lock()
                    .map_err(|_| anyhow::anyhow!("runtime busy"))?;
                let msg = child_failure(&dir, "nexus-kvmd");
                g.last_error = Some(msg.clone());
                ui_log(&dir, &format!("daemon socket missing: {msg}"));
                anyhow::bail!(msg);
            }
            ui_log(&dir, "host runtime started");
        }
        Role::Client => {
            client_preflight()?;
            let bin = find_bin(app, "rkvm-client").ok_or_else(|| {
                anyhow::anyhow!(
                    "rkvm-client not found. Build it with: cargo build -p rkvm-client --manifest-path rkvm-master/Cargo.toml"
                )
            })?;
            let cfg = client_config_path(&dir);
            if !cfg.is_file() {
                anyhow::bail!("missing client.toml; reconnect using the primary machine's invite code");
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
            let server = state.server.clone().or_else(|| {
                fs::read_to_string(client_config_path(&dir)).ok().and_then(|t| {
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
        .map_err(|_| anyhow::anyhow!("no password; configure this machine as primary"))?;
    let certificate = fs::read_to_string(cert_path(&dir))
        .map_err(|_| anyhow::anyhow!("no certificate yet"))?;
    Ok(Invite {
        server: advertise(),
        password: password.trim().into(),
        certificate,
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
    DaemonClient {
        socket: socket_path().to_string_lossy().into(),
    }
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
}
