use crate::persist;
use crate::runtime::{self, AppRuntime};
use crate::state::AppLifecycleState;
#[cfg(not(target_os = "linux"))]
use crate::window_labels::MAIN_TRAY_ID;
use crate::windows;
use std::sync::atomic::Ordering;
#[cfg(not(target_os = "linux"))]
use std::sync::Arc;
#[cfg(not(target_os = "linux"))]
use tauri::menu::{Menu, MenuItem};
#[cfg(not(target_os = "linux"))]
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, RunEvent};

/// Confirm quit. Returns true if the user accepted stopping the service and exiting.
pub fn confirm_quit() -> bool {
    rfd::MessageDialog::new()
        .set_title("Quit NexusKVM?")
        .set_description(
            "This stops keyboard/mouse sharing on this machine and exits the app.\n\n\
             Tip: closing the window with ✕ only hides NexusKVM in the system tray \
             and keeps the service running.",
        )
        .set_buttons(rfd::MessageButtons::YesNo)
        .set_level(rfd::MessageLevel::Warning)
        .show()
        == rfd::MessageDialogResult::Yes
}

pub fn quit_app(app: &AppHandle, rt: &AppRuntime) {
    if !confirm_quit() {
        return;
    }
    rt.shutdown();
    // Best-effort: stop boot/GDM units without prompting (may no-op without privileges).
    for unit in ["nexuskvm-host.service", "nexuskvm-client.service"] {
        let _ = std::process::Command::new("systemctl")
            .args(["--no-ask-password", "stop", unit])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }

    let state = app.state::<AppLifecycleState>();
    state.quitting.store(true, Ordering::SeqCst);
    app.exit(0);
}

/// GTK/appindicator tray. Not used on Linux (see linux_tray.rs): the
/// libappindicator backend cannot deliver icon click events.
#[cfg(not(target_os = "linux"))]
pub fn create_tray(app: &mut tauri::App, rt: Arc<AppRuntime>) -> tauri::Result<()> {
    let handle = app.handle();
    let panel_i = MenuItem::with_id(handle, "open_tray", "Open Quick Panel", true, None::<&str>)?;
    let show_i = MenuItem::with_id(handle, "show", "Open Main Window", true, None::<&str>)?;
    let quit_i = MenuItem::with_id(handle, "quit", "Quit NexusKVM", true, None::<&str>)?;
    let menu = Menu::with_items(handle, &[&panel_i, &show_i, &quit_i])?;

    let icon = match app.default_window_icon() {
        Some(i) => i.clone(),
        None => tauri::image::Image::from_bytes(include_bytes!("../icons/32x32.png"))
            .unwrap_or_else(|_| panic!("Failed to load tray icon")),
    };

    let rt_menu = rt.clone();
    let builder = TrayIconBuilder::with_id(MAIN_TRAY_ID)
        .icon(icon)
        .menu(&menu)
        .tooltip("NexusKVM — Spatial KVM")
        .show_menu_on_left_click(false);

    builder
        .on_menu_event(move |app_handle, event| match event.id.as_ref() {
            "open_tray" => {
                let _ = windows::toggle_tray_panel(app_handle);
            }
            "show" => {
                let _ = windows::open_main_window(app_handle);
            }
            "quit" => quit_app(app_handle, &rt_menu),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            tauri_plugin_positioner::on_tray_event(tray.app_handle(), &event);
            // Toggle strictly on release: matching Down+Up fired the toggle
            // twice per click (show→hide instantly).
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let _ = windows::toggle_tray_panel(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

pub fn handle_run_event(app: &AppHandle, event: &RunEvent, rt: &AppRuntime) {
    match event {
        RunEvent::ExitRequested { api, .. } => {
            let state = app.state::<AppLifecycleState>();
            let quitting = state.quitting.load(Ordering::SeqCst);
            if !quitting {
                api.prevent_exit();
                let _ = windows::hide_main_window(app);
            }
        }
        RunEvent::Exit => {
            rt.shutdown();
        }
        _ => {}
    }
}

pub fn ensure_persistence(app: &AppHandle) {
    if let Ok(dir) = runtime::data_dir(app) {
        let _ = persist::ensure_session_persistence(app, &dir);
    }
}
