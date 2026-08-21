use crate::persist;
use crate::runtime::{self, AppRuntime};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, RunEvent, WindowEvent,
};

/// When true, the next exit is allowed (Quit confirmed).
static ALLOW_EXIT: AtomicBool = AtomicBool::new(false);

pub fn allow_exit() {
    ALLOW_EXIT.store(true, Ordering::SeqCst);
}

pub fn exit_allowed() -> bool {
    ALLOW_EXIT.load(Ordering::SeqCst)
}

pub fn show_main_window(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    }
}

pub fn hide_to_tray(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.hide();
    }
}

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
    allow_exit();
    app.exit(0);
}

pub fn setup_tray(app: &AppHandle, rt: Arc<AppRuntime>) -> tauri::Result<()> {
    let show_i = MenuItem::with_id(app, "show", "Show NexusKVM", true, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "quit", "Quit…", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_i, &quit_i])?;

    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or_else(|| tauri::Error::WindowNotFound)?;

    let rt_menu = rt.clone();
    let _tray = TrayIconBuilder::with_id("nexuskvm")
        .icon(icon)
        .menu(&menu)
        .tooltip("NexusKVM")
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "show" => show_main_window(app),
            "quit" => quit_app(app, &rt_menu),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            // Linux StatusNotifier often only supports the menu; still handle clicks where available.
            if let TrayIconEvent::Click {
                button: tauri::tray::MouseButton::Left,
                button_state: tauri::tray::MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

pub fn attach_window_close_handler(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let handle = app.clone();
        win.on_window_event(move |event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                hide_to_tray(&handle);
            }
        });
    }
}

pub fn handle_run_event(app: &AppHandle, event: &RunEvent, rt: &AppRuntime) {
    match event {
        RunEvent::ExitRequested { api, .. } => {
            if !exit_allowed() {
                api.prevent_exit();
                hide_to_tray(app);
            }
        }
        RunEvent::Exit => {
            if exit_allowed() {
                rt.shutdown();
            }
        }
        _ => {}
    }
}

pub fn ensure_persistence(app: &AppHandle) {
    if let Ok(dir) = runtime::data_dir(app) {
        // Autostart only — never pkexec on every login.
        let _ = persist::ensure_session_persistence(app, &dir);
    }
}
