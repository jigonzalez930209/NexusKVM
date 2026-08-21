//! Keep host/client pairing across logout and reboot.
//!
//! - XDG autostart: reopen the tray UI after graphical login (no privileges)
//! - system units via pkexec: only once, when the role is first set up (GDM / boot)

use crate::runtime::{
    client_config_path, daemon_config_path, load_state, ui_log, Role, SavedState,
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
use tauri::{AppHandle, Manager};

fn autostart_dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".config/autostart"))
}

fn exe_path(app: &AppHandle) -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .or_else(|| app.path().resource_dir().ok().map(|d| d.join("nexuskvm")))
}

fn unit_name(role: Role) -> &'static str {
    match role {
        Role::Host => "nexuskvm-host.service",
        Role::Client => "nexuskvm-client.service",
    }
}

fn systemctl_quiet(args: &[&str]) -> bool {
    Command::new("systemctl")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Session-only persistence (autostart desktop entry). Never prompts for a password.
pub fn ensure_session_persistence(app: &AppHandle, dir: &Path) -> anyhow::Result<()> {
    if load_state(dir).is_none() {
        return Ok(());
    }
    install_autostart(app, dir)
}

/// Called after first pairing only: enable the boot/GDM unit if it is not already on.
pub fn install_persistence(app: &AppHandle, dir: &Path) -> anyhow::Result<()> {
    let Some(state) = load_state(dir) else {
        return Ok(());
    };
    install_autostart(app, dir)?;
    let _ = enable_boot_service_once(app, dir, &state);
    Ok(())
}

fn install_autostart(app: &AppHandle, dir: &Path) -> anyhow::Result<()> {
    let Some(exe) = exe_path(app) else {
        return Ok(());
    };
    let Some(auto_dir) = autostart_dir() else {
        return Ok(());
    };
    fs::create_dir_all(&auto_dir)?;
    let desktop = auto_dir.join("nexuskvm.desktop");
    let body = format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=NexusKVM\n\
         Comment=Keep keyboard/mouse sharing ready after login\n\
         Exec=\"{}\" --tray\n\
         Icon=nexuskvm\n\
         Terminal=false\n\
         Categories=Utility;Network;\n\
         X-GNOME-Autostart-enabled=true\n\
         StartupNotify=false\n",
        exe.display()
    );
    fs::write(&desktop, body)?;
    ui_log(dir, &format!("autostart written: {}", desktop.display()));
    Ok(())
}

fn enable_boot_service_once(app: &AppHandle, dir: &Path, state: &SavedState) -> anyhow::Result<()> {
    let unit = unit_name(state.role);
    // Already set up → never ask for a password again on login/start.
    if boot_service_enabled(state.role) || boot_service_active(state.role) {
        ui_log(
            dir,
            &format!("{unit} already enabled/active; skipping pkexec"),
        );
        return Ok(());
    }

    let mut scripts = Vec::new();
    if let Ok(res) = app.path().resource_dir() {
        scripts.push(res.join("nexuskvm-enable-boot.sh"));
    }
    if let Some(manifest) = option_env!("CARGO_MANIFEST_DIR") {
        scripts.push(Path::new(manifest).join("../scripts/nexuskvm-enable-boot.sh"));
    }
    scripts.push(PathBuf::from(
        "/usr/libexec/nexuskvm/nexuskvm-enable-boot.sh",
    ));

    let Some(script) = scripts.into_iter().find(|p| p.is_file()) else {
        ui_log(dir, "boot persist script not found (optional for GDM)");
        return Ok(());
    };

    let role = match state.role {
        Role::Host => "host",
        Role::Client => "client",
    };

    match state.role {
        Role::Host if !daemon_config_path(dir).is_file() => return Ok(()),
        Role::Client if !client_config_path(dir).is_file() => return Ok(()),
        _ => {}
    }

    ui_log(dir, "requesting elevation once to enable boot/GDM service…");
    let status = Command::new("pkexec")
        .arg(&script)
        .arg(role)
        .arg(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    match status {
        Ok(s) if s.success() => ui_log(dir, &format!("boot service enabled for {role}")),
        Ok(s) => ui_log(dir, &format!("boot service skipped (exit {:?})", s.code())),
        Err(e) => ui_log(dir, &format!("pkexec not available: {e}")),
    }
    Ok(())
}

pub fn clear_persistence(_app: &AppHandle) {
    if let Some(dir) = autostart_dir() {
        let _ = fs::remove_file(dir.join("nexuskvm.desktop"));
    }
    for unit in ["nexuskvm-host.service", "nexuskvm-client.service"] {
        if !systemctl_quiet(&["is-enabled", "--quiet", unit])
            && !systemctl_quiet(&["is-active", "--quiet", unit])
        {
            continue;
        }
        let _ = Command::new("pkexec")
            .args(["systemctl", "disable", "--now", unit])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

/// True when argv requests starting hidden in the tray (autostart).
pub fn start_hidden_from_args() -> bool {
    std::env::args().any(|a| a == "--tray" || a == "--hidden")
}

/// Whether a system/boot unit already owns this role (avoid double spawn).
pub fn boot_service_active(role: Role) -> bool {
    systemctl_quiet(&["is-active", "--quiet", unit_name(role)])
}

pub fn boot_service_enabled(role: Role) -> bool {
    systemctl_quiet(&["is-enabled", "--quiet", unit_name(role)])
}

/// Control socket used by the session agent / UI.
pub fn control_socket_path() -> PathBuf {
    let system = PathBuf::from("/run/nexuskvm/control.sock");
    // Prefer the system daemon whenever its unit is up, even if the user cannot
    // `stat` the parent directory yet (permissions are fixed by Group=input).
    if boot_service_active(Role::Host) {
        return system;
    }
    if system.exists() {
        return system;
    }
    let base = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| {
        format!(
            "/tmp/nexuskvm-{}",
            std::env::var("USER").unwrap_or_else(|_| "user".into())
        )
    });
    PathBuf::from(base).join("nexuskvm/control.sock")
}
