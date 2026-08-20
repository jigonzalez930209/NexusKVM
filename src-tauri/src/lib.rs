mod runtime;

use nexus_common::*;
use runtime::{AppRuntime, Invite, RuntimeSnapshot};
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
async fn switch_target(target: String) -> Result<AppStatus, String> {
    status_from(
        runtime::control_client()
            .send(ControlCommand::Switch {
                target,
                entry: Some(EntryPoint {
                    edge: Edge::Left,
                    normalized_position: 0.5,
                    inset_px: 6,
                }),
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
    Ok(match file.peer_side {
        PeerSide::Left => "left".into(),
        PeerSide::Right => "right".into(),
        PeerSide::Top => "top".into(),
        PeerSide::Bottom => "bottom".into(),
    })
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let runtime = Arc::new(AppRuntime::new());
    let runtime_exit = runtime.clone();
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(runtime)
        .invoke_handler(tauri::generate_handler![
            open_logs,
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
            get_peer_side
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            let rt = (*app.state::<Arc<AppRuntime>>()).clone();
            tauri::async_runtime::spawn(async move {
                if runtime::data_dir(&handle).ok().is_some() {
                    let _ = runtime::start(&handle, &rt).await;
                }
            });
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error al ejecutar NexusKVM")
        .run(move |_app, event| {
            if let tauri::RunEvent::Exit = event {
                runtime_exit.shutdown();
            }
        });
}
