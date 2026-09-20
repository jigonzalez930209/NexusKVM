mod linux_tray;
mod metrics;
mod persist;
mod runtime;
mod state;
mod tray;
mod window_labels;
mod windows;

use nexus_agent::peer_channel::{self, PeerMessage};
use nexus_common::*;
use runtime::{AppRuntime, Invite, Role, RuntimeSnapshot};
use state::AppLifecycleState;
use std::collections::BTreeMap;
use std::sync::Arc;
use tauri::{Emitter, Manager, State};

fn map_err(e: impl ToString) -> String {
    e.to_string()
}

async fn status_from(r: ControlResponse) -> Result<AppStatus, String> {
    if r.ok {
        r.status.ok_or_else(|| "response missing status".into())
    } else {
        Err(r.error.unwrap_or_else(|| "unknown error".into()))
    }
}

#[tauri::command]
fn open_logs(app: tauri::AppHandle) -> Result<String, String> {
    runtime::open_logs(&app).map_err(map_err)
}

#[tauri::command]
fn start_dragging(window: tauri::Window) {
    // Drag the calling window (main OR tray-panel topbar).
    let _ = window.start_dragging();
}

#[tauri::command]
fn hide_window(app: tauri::AppHandle) {
    let _ = windows::hide_main_window(&app);
}

#[tauri::command]
fn minimize_window(app: tauri::AppHandle) {
    if let Some(win) = app.get_webview_window(window_labels::MAIN_WINDOW) {
        let _ = win.minimize();
    }
}

#[tauri::command]
fn toggle_maximize(app: tauri::AppHandle) {
    if let Some(win) = app.get_webview_window(window_labels::MAIN_WINDOW) {
        if let Ok(is_max) = win.is_maximized() {
            if is_max {
                let _ = win.unmaximize();
            } else {
                let _ = win.maximize();
            }
        }
    }
}

#[tauri::command]
fn position_tray_panel(app: tauri::AppHandle) {
    let _ = windows::position_tray_panel(&app);
}

#[tauri::command]
fn toggle_tray_panel(app: tauri::AppHandle) {
    let _ = windows::toggle_tray_panel(&app);
}

#[tauri::command]
fn toggle_tray_window(app: tauri::AppHandle) {
    let _ = windows::toggle_tray_panel(&app);
}

#[tauri::command]
fn open_main_window(app: tauri::AppHandle) -> Result<(), String> {
    windows::open_main_window(&app)
}

#[tauri::command]
fn show_main_window_cmd(app: tauri::AppHandle) {
    let _ = windows::open_main_window(&app);
}

#[tauri::command]
fn hide_tray_panel(app: tauri::AppHandle) {
    let _ = windows::hide_tray_panel(&app);
}

#[tauri::command]
fn hide_tray_window(app: tauri::AppHandle) {
    let _ = windows::hide_tray_panel(&app);
}

#[tauri::command]
fn quit_app_cmd(app: tauri::AppHandle, rt: State<'_, Arc<AppRuntime>>) {
    tray::quit_app(&app, &rt);
}

#[tauri::command]
async fn runtime_status(
    app: tauri::AppHandle,
    rt: State<'_, Arc<AppRuntime>>,
) -> Result<RuntimeSnapshot, String> {
    Ok(runtime::snapshot(&app, &rt).await)
}

#[tauri::command]
async fn setup_as_host(
    app: tauri::AppHandle,
    rt: State<'_, Arc<AppRuntime>>,
) -> Result<RuntimeSnapshot, String> {
    runtime::setup_host(&app, &rt).await.map_err(map_err)
}

#[tauri::command]
async fn setup_as_client(
    app: tauri::AppHandle,
    rt: State<'_, Arc<AppRuntime>>,
    invite: Invite,
) -> Result<RuntimeSnapshot, String> {
    runtime::setup_client(&app, &rt, invite)
        .await
        .map_err(map_err)
}

#[tauri::command]
async fn start_runtime(
    app: tauri::AppHandle,
    rt: State<'_, Arc<AppRuntime>>,
) -> Result<RuntimeSnapshot, String> {
    runtime::start(&app, &rt).await.map_err(map_err)?;
    Ok(runtime::snapshot(&app, &rt).await)
}

#[tauri::command]
async fn stop_runtime(
    app: tauri::AppHandle,
    rt: State<'_, Arc<AppRuntime>>,
) -> Result<RuntimeSnapshot, String> {
    rt.shutdown();
    persist::stop_boot_services();
    Ok(runtime::snapshot(&app, &rt).await)
}

#[tauri::command]
async fn reset_runtime(
    app: tauri::AppHandle,
    rt: State<'_, Arc<AppRuntime>>,
) -> Result<RuntimeSnapshot, String> {
    runtime::reset_setup(&app, &rt).map_err(map_err)?;
    Ok(runtime::snapshot(&app, &rt).await)
}

#[tauri::command]
async fn pairing_invite(app: tauri::AppHandle) -> Result<Invite, String> {
    let is_client = runtime::data_dir(&app)
        .ok()
        .and_then(|d| runtime::load_state(&d))
        .map(|s| s.role == Role::Client)
        .unwrap_or(false);
    if is_client {
        return Err("only the host can generate a pairing invite".into());
    }
    runtime::invite(&app).map_err(map_err)
}

#[tauri::command]
async fn daemon_status() -> Result<AppStatus, String> {
    status_from(
        runtime::control_client()
            .send(ControlCommand::Status)
            .await
            .map_err(map_err)?,
    )
    .await
}

#[tauri::command]
async fn switch_target(app: tauri::AppHandle, target: String) -> Result<AppStatus, String> {
    // Crossing this machine's layout edge must enter the remote on the opposite side.
    let entry = runtime::get_layout(&app)
        .map(|f| entry_for(f.peer_side.as_edge(), 0.5))
        .unwrap_or_else(|_| entry_for(Edge::Right, 0.5));
    let status = status_from(
        runtime::control_client()
            .send(ControlCommand::Switch {
                target,
                entry: Some(entry),
            })
            .await
            .map_err(map_err)?,
    )
    .await?;
    let _ = app.emit("nexus-target-changed", &status.active_target);
    let _ = app.emit("nexus-status-changed", &status);
    Ok(status)
}

#[tauri::command]
async fn switch_local(app: tauri::AppHandle) -> Result<AppStatus, String> {
    let status = status_from(
        runtime::control_client()
            .send(ControlCommand::Local)
            .await
            .map_err(map_err)?,
    )
    .await?;
    let _ = app.emit("nexus-target-changed", &status.active_target);
    let _ = app.emit("nexus-status-changed", &status);
    Ok(status)
}

#[tauri::command]
async fn release_all() -> Result<(), String> {
    let r = runtime::control_client()
        .send(ControlCommand::ReleaseAll)
        .await
        .map_err(map_err)?;
    if r.ok {
        Ok(())
    } else {
        Err(r.error.unwrap_or_default())
    }
}

#[tauri::command]
fn set_peer_side(app: tauri::AppHandle, side: String) -> Result<String, String> {
    let file = runtime::set_peer_side(&app, &side).map_err(map_err)?;
    let side_str = match file.peer_side {
        PeerSide::Left => "left",
        PeerSide::Right => "right",
        PeerSide::Top => "top",
        PeerSide::Bottom => "bottom",
    };
    let _ = windows::position_edge_portal(&app, Some(side_str));
    let _ = app.emit("nexus-peer-side-changed", side_str);
    Ok(side_str.into())
}

#[tauri::command]
fn get_peer_side(app: tauri::AppHandle) -> Result<String, String> {
    let file = runtime::get_layout(&app).map_err(map_err)?;
    Ok(match file.peer_side {
        PeerSide::Left => "left".into(),
        PeerSide::Right => "right".into(),
        PeerSide::Top => "top".into(),
        PeerSide::Bottom => "bottom".into(),
    })
}

#[tauri::command]
async fn switch_edge(app: tauri::AppHandle, normalized_position: f32) -> Result<AppStatus, String> {
    let _ = normalized_position;
    let dir = runtime::data_dir(&app).ok();
    let is_client = runtime::data_dir(&app)
        .ok()
        .and_then(|d| runtime::load_state(&d))
        .map(|s| s.role == Role::Client)
        .unwrap_or(false);
    if let Some(d) = dir.as_deref() {
        runtime::ui_log(
            d,
            &format!(
                "switch_edge start role={} pos={normalized_position:.3}",
                if is_client { "client" } else { "host" }
            ),
        );
    }

    let status = if is_client {
        let server = runtime::data_dir(&app)
            .ok()
            .and_then(|d| runtime::load_state(&d))
            .and_then(|s| s.server);
        let addr = server
            .as_deref()
            .and_then(peer_channel::control_addr_from_peer)
            .ok_or_else(|| "no host address to return control".to_string())?;
        let dir = runtime::data_dir(&app).map_err(map_err)?;
        let password = std::fs::read_to_string(dir.join("password"))
            .map_err(|_| "no pairing password".to_string())?;
        let ack = peer_channel::send_to(addr, &PeerMessage::SwitchLocal, password.trim())
            .await
            .map_err(map_err)?;
        match ack {
            PeerMessage::Ack {
                ok: true,
                active_target,
                ..
            } => AppStatus {
                state: RuntimeState::Local,
                active_target: active_target.unwrap_or_else(|| LOCAL_TARGET.into()),
                peers: BTreeMap::new(),
                agent_connected: false,
                portal_available: false,
                emergency_shortcut: "Left Alt + Left Ctrl".into(),
            },
            PeerMessage::Ack {
                ok: false, error, ..
            } => {
                return Err(error.unwrap_or_else(|| "host did not return local".into()));
            }
            _ => return Err("unexpected peer control reply".into()),
        }
    } else {
        // Dedicated edge-crossing command: the daemon picks the connected peer
        // and applies containment (no-op while already remote or inside the
        // debounce window), so a duplicated portal event can never cycle
        // control straight back to this machine.
        let side = runtime::get_layout(&app)
            .map(|f| f.peer_side)
            .unwrap_or(nexus_common::PeerSide::Right);
        let edge = match side {
            nexus_common::PeerSide::Left => Edge::Left,
            nexus_common::PeerSide::Right => Edge::Right,
            nexus_common::PeerSide::Top => Edge::Top,
            nexus_common::PeerSide::Bottom => Edge::Bottom,
        };
        let response = match runtime::control_client()
            .send(ControlCommand::SwitchEdge {
                side: edge,
                position: normalized_position,
            })
            .await
        {
            Ok(r) => r,
            Err(e) => {
                if let Some(d) = dir.as_deref() {
                    runtime::ui_log(d, &format!("switch_edge daemon error: {e}"));
                }
                return Err(map_err(e));
            }
        };
        match status_from(response).await {
            Ok(s) => s,
            Err(e) => {
                if let Some(d) = dir.as_deref() {
                    runtime::ui_log(d, &format!("switch_edge rejected: {e}"));
                }
                return Err(e);
            }
        }
    };
    if let Some(d) = dir.as_deref() {
        runtime::ui_log(
            d,
            &format!("switch_edge done target={}", status.active_target),
        );
    }
    let _ = app.emit("nexus-target-changed", &status.active_target);
    let _ = app.emit("nexus-status-changed", &status);
    Ok(status)
}

#[tauri::command]
fn position_edge_portal_cmd(app: tauri::AppHandle, side: Option<String>) {
    let _ = windows::position_edge_portal(&app, side.as_deref());
}

fn maybe_show_edge_portal(app: &tauri::AppHandle) {
    // Always keep the X11 edge strip visible as a fallback/visual cue.
    // GNOME InputCapture may report available while still failing to deliver
    // activations (common on portal v1); the blue bar then remains the switch path.
    let _ = windows::show_edge_portal(app);
}

#[tauri::command]
fn show_edge_portal_cmd(app: tauri::AppHandle) {
    maybe_show_edge_portal(&app);
}

#[tauri::command]
fn hide_edge_portal_cmd(app: tauri::AppHandle) {
    let _ = windows::hide_edge_portal(&app);
}

#[tauri::command]
fn toggle_edge_portal(app: tauri::AppHandle, enable: bool) {
    if enable {
        let _ = windows::show_edge_portal(&app);
    } else {
        let _ = windows::hide_edge_portal(&app);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(target_os = "linux")]
    {
        std::env::set_var("GDK_BACKEND", "x11");
    }

    let runtime = Arc::new(AppRuntime::new());
    let runtime_exit = runtime.clone();
    let start_hidden = persist::start_hidden_from_args();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_positioner::init())
        .manage(AppLifecycleState::default())
        .manage(runtime)
        .invoke_handler(tauri::generate_handler![
            open_logs,
            start_dragging,
            hide_window,
            minimize_window,
            toggle_maximize,
            position_tray_panel,
            toggle_tray_panel,
            toggle_tray_window,
            open_main_window,
            show_main_window_cmd,
            hide_tray_panel,
            hide_tray_window,
            quit_app_cmd,
            runtime_status,
            setup_as_host,
            setup_as_client,
            start_runtime,
            stop_runtime,
            reset_runtime,
            pairing_invite,
            daemon_status,
            switch_target,
            switch_local,
            release_all,
            set_peer_side,
            get_peer_side,
            switch_edge,
            position_edge_portal_cmd,
            show_edge_portal_cmd,
            hide_edge_portal_cmd,
            toggle_edge_portal
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            let rt = (*app.state::<Arc<AppRuntime>>()).clone();

            // Linux: ksni SNI tray (libappindicator can't deliver clicks).
            #[cfg(target_os = "linux")]
            linux_tray::create_tray(app, rt.clone())?;
            #[cfg(not(target_os = "linux"))]
            tray::create_tray(app, rt.clone())?;

            windows::configure_main_window(app)?;
            windows::configure_tray_panel(app)?;
            windows::configure_edge_portal(app)?;

            maybe_show_edge_portal(&handle);

            if start_hidden {
                let _ = windows::hide_main_window(&handle);
            }

            let watcher_handle = handle.clone();
            tauri::async_runtime::spawn(async move {
                let mut last_target: Option<String> = None;
                let mut last_side: Option<String> = None;
                let mut ticks: u32 = 0;
                loop {
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                    ticks = ticks.wrapping_add(1);
                    if let Ok(st) = runtime::control_client().send(ControlCommand::Status).await {
                        if let Some(status) = st.status {
                            if last_target.as_deref() != Some(&status.active_target) {
                                last_target = Some(status.active_target.clone());
                                let _ = watcher_handle
                                    .emit("nexus-target-changed", &status.active_target);
                                let _ = watcher_handle.emit("nexus-status-changed", &status);
                            }
                        }
                    }
                    if let Ok(f) = runtime::get_layout(&watcher_handle) {
                        let side = match f.peer_side {
                            PeerSide::Left => "left",
                            PeerSide::Right => "right",
                            PeerSide::Top => "top",
                            PeerSide::Bottom => "bottom",
                        };
                        if last_side.as_deref() != Some(side) {
                            last_side = Some(side.to_string());
                            let _ = watcher_handle.emit("nexus-peer-side-changed", side);
                            maybe_show_edge_portal(&watcher_handle);
                        }
                    }
                    // Periodic safety re-assert (every ~5s) in case the WM hid
                    // the strip. Window ops are marshaled to the main thread by
                    // windows.rs; never call GTK from this tokio worker directly.
                    if ticks.is_multiple_of(25) {
                        maybe_show_edge_portal(&watcher_handle);
                    }
                }
            });

            tauri::async_runtime::spawn(async move {
                if runtime::data_dir(&handle).ok().is_some() {
                    let _ = runtime::start(&handle, &rt).await;
                    runtime::spawn_supervisor(handle.clone(), rt.clone());
                    tray::ensure_persistence(&handle);
                }
            });
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("failed to run NexusKVM")
        .run(move |app, event| {
            tray::handle_run_event(app, &event, &runtime_exit);
        });
}
