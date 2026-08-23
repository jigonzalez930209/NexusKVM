mod linux_tray;
mod metrics;
mod persist;
mod runtime;
mod state;
mod tray;
mod window_labels;
mod windows;

use nexus_common::*;
use runtime::{AppRuntime, Invite, RuntimeSnapshot};
use state::AppLifecycleState;
use std::sync::Arc;
use tauri::{Manager, State};

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
    // Use the edge configured in the stored layout so the cursor enters the
    // remote screen where the user placed it, not a hardcoded side.
    let entry = runtime::get_layout(&app)
        .map(|f| EntryPoint {
            edge: match f.peer_side {
                PeerSide::Left => Edge::Left,
                PeerSide::Right => Edge::Right,
                PeerSide::Top => Edge::Top,
                PeerSide::Bottom => Edge::Bottom,
            },
            normalized_position: 0.5,
            inset_px: 6,
        })
        .unwrap_or(EntryPoint {
            edge: Edge::Right,
            normalized_position: 0.5,
            inset_px: 6,
        });
    status_from(
        runtime::control_client()
            .send(ControlCommand::Switch {
                target,
                entry: Some(entry),
            })
            .await
            .map_err(map_err)?,
    )
    .await
}

#[tauri::command]
async fn switch_local() -> Result<AppStatus, String> {
    status_from(
        runtime::control_client()
            .send(ControlCommand::Local)
            .await
            .map_err(map_err)?,
    )
    .await
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
    let layout = runtime::get_layout(&app).map_err(map_err)?;
    let is_client = runtime::data_dir(&app)
        .ok()
        .and_then(|d| runtime::load_state(&d))
        .map(|s| s.role == runtime::Role::Client)
        .unwrap_or(false);

    if is_client {
        status_from(
            runtime::control_client()
                .send(ControlCommand::Local)
                .await
                .map_err(map_err)?,
        )
        .await
    } else {
        let remote_target = layout.remote_peer.clone().unwrap_or_else(|| "peer".into());
        let entry = EntryPoint {
            edge: match layout.peer_side {
                PeerSide::Left => Edge::Left,
                PeerSide::Right => Edge::Right,
                PeerSide::Top => Edge::Top,
                PeerSide::Bottom => Edge::Bottom,
            },
            normalized_position: normalized_position.clamp(0.0, 1.0),
            inset_px: 6,
        };
        status_from(
            runtime::control_client()
                .send(ControlCommand::Switch {
                    target: remote_target,
                    entry: Some(entry),
                })
                .await
                .map_err(map_err)?,
        )
        .await
    }
}

#[tauri::command]
fn position_edge_portal_cmd(app: tauri::AppHandle, side: Option<String>) {
    let _ = windows::position_edge_portal(&app, side.as_deref());
}

#[tauri::command]
fn show_edge_portal_cmd(app: tauri::AppHandle) {
    let _ = windows::show_edge_portal(&app);
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

            let _ = windows::show_edge_portal(&handle);

            if start_hidden {
                let _ = windows::hide_main_window(&handle);
            }

            tauri::async_runtime::spawn(async move {
                if runtime::data_dir(&handle).ok().is_some() {
                    let _ = runtime::start(&handle, &rt).await;
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
