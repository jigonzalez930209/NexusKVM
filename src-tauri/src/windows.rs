use crate::state::AppLifecycleState;
use crate::window_labels::{MAIN_WINDOW, TRAY_PANEL_WINDOW};
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Manager, WindowEvent};

/// Quick panel geometry: fixed size, pinned to the top-right corner of the
/// monitor — 10 logical px from the right edge, 50 from the top.
const PANEL_W: f64 = 360.0;
const PANEL_H: f64 = 560.0;
const OFFSET_RIGHT: f64 = 10.0;
const OFFSET_TOP: f64 = 50.0;

pub fn position_tray_panel(app: &AppHandle) -> Result<(), String> {
    let panel = app
        .get_webview_window(TRAY_PANEL_WINDOW)
        .ok_or_else(|| format!("Window '{TRAY_PANEL_WINDOW}' not found"))?;

    // Fixed size on every open.
    let _ = panel.set_size(tauri::Size::Logical(tauri::LogicalSize {
        width: PANEL_W,
        height: PANEL_H,
    }));

    // Monitor lookup: check panel's current monitor, app's primary monitor, or available monitors
    let monitor = panel
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| app.primary_monitor().ok().flatten())
        .or_else(|| panel.primary_monitor().ok().flatten())
        .or_else(|| {
            app.available_monitors()
                .ok()
                .and_then(|mut m| m.pop())
        });

    if let Some(monitor) = monitor {
        let scale = monitor.scale_factor();
        let origin = monitor.position();
        let size = monitor.size();

        // Compute physical coordinates directly to avoid any DPI conversion inaccuracies
        let target_x = origin.x + (size.width as i32 - ((PANEL_W + OFFSET_RIGHT) * scale) as i32).max(0);
        let target_y = origin.y + (OFFSET_TOP * scale) as i32;

        let _ = panel.set_position(tauri::Position::Physical(tauri::PhysicalPosition {
            x: target_x,
            y: target_y,
        }));
    } else {
        // Fallback
        let _ = panel.set_position(tauri::Position::Logical(tauri::LogicalPosition {
            x: 0.0,
            y: OFFSET_TOP,
        }));
    }

    Ok(())
}

/// Place the panel at the top-right of the screen and ensure window manager
/// centering does not override the placement.
pub fn show_tray_panel(app: &AppHandle) -> Result<(), String> {
    let panel = app
        .get_webview_window(TRAY_PANEL_WINDOW)
        .ok_or_else(|| format!("Window '{TRAY_PANEL_WINDOW}' not found"))?;

    let _ = position_tray_panel(app);
    let _ = panel.unminimize();
    panel.show().map_err(|e| e.to_string())?;
    let _ = position_tray_panel(app);
    let _ = panel.set_focus();

    // On Linux window managers (GNOME/Mutter, XFCE, KWin), the WM asynchronously
    // applies its default window placement policy upon map. We re-apply position
    // after small intervals so the panel stays locked to the top-right.
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;
        let _ = position_tray_panel(&app_handle);
        tokio::time::sleep(std::time::Duration::from_millis(70)).await;
        let _ = position_tray_panel(&app_handle);
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        let _ = position_tray_panel(&app_handle);
    });

    Ok(())
}

pub fn hide_tray_panel(app: &AppHandle) -> Result<(), String> {
    let panel = app
        .get_webview_window(TRAY_PANEL_WINDOW)
        .ok_or_else(|| format!("Window '{TRAY_PANEL_WINDOW}' not found"))?;

    panel.hide().map_err(|e| e.to_string())
}

pub fn toggle_tray_panel(app: &AppHandle) -> Result<(), String> {
    let panel = app
        .get_webview_window(TRAY_PANEL_WINDOW)
        .ok_or_else(|| format!("Window '{TRAY_PANEL_WINDOW}' not found"))?;

    let visible = panel.is_visible().map_err(|e| e.to_string())?;

    if visible {
        hide_tray_panel(app)
    } else {
        show_tray_panel(app)
    }
}

pub fn open_main_window(app: &AppHandle) -> Result<(), String> {
    let main = app
        .get_webview_window(MAIN_WINDOW)
        .ok_or_else(|| format!("Window '{MAIN_WINDOW}' not found"))?;

    main.show().map_err(|e| e.to_string())?;
    main.unminimize().map_err(|e| e.to_string())?;
    main.set_focus().map_err(|e| e.to_string())?;

    // The tray panel is an independent surface: it stays open.
    Ok(())
}

pub fn hide_main_window(app: &AppHandle) -> Result<(), String> {
    let main = app
        .get_webview_window(MAIN_WINDOW)
        .ok_or_else(|| format!("Window '{MAIN_WINDOW}' not found"))?;

    main.hide().map_err(|e| e.to_string())
}

pub fn configure_main_window(app: &mut tauri::App) -> tauri::Result<()> {
    if let Some(main) = app.get_webview_window(MAIN_WINDOW) {
        let app_handle = app.handle().clone();
        let main_for_event = main.clone();

        main.on_window_event(move |event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let state = app_handle.state::<AppLifecycleState>();
                let quitting = state.quitting.load(Ordering::SeqCst);

                if !quitting {
                    api.prevent_close();
                    let _ = main_for_event.hide();
                }
            }
        });
    }

    Ok(())
}

pub fn configure_tray_panel(app: &mut tauri::App) -> tauri::Result<()> {
    if let Some(panel) = app.get_webview_window(TRAY_PANEL_WINDOW) {
        // Independent window: no auto-hide on focus loss (clicking the main
        // window must not close it). Close = X button, Escape, or tray toggle.
        let app_handle = app.handle().clone();
        panel.on_window_event(move |event| match event {
            WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                if let Some(p) = app_handle.get_webview_window(TRAY_PANEL_WINDOW) {
                    let _ = p.hide();
                }
            }
            _ => {}
        });
    }

    Ok(())
}
