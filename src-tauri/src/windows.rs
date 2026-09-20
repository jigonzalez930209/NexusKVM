use crate::state::AppLifecycleState;
use crate::window_labels::{EDGE_PORTAL_WINDOW, MAIN_WINDOW, TRAY_PANEL_WINDOW};
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Manager, WindowEvent};

/// Quick panel geometry: fixed size, pinned to the top-right corner of the
/// monitor — 10 logical px from the right edge, 50 from the top.
const PANEL_W: f64 = 360.0;
const PANEL_H: f64 = 560.0;
const OFFSET_RIGHT: f64 = 10.0;
const OFFSET_TOP: f64 = 50.0;

/// Run a GTK/window operation on the main thread.
///
/// GDK is not thread-safe: `available_monitors()` / `current_monitor()` and
/// every `WebviewWindow` show/hide/set_position call must happen on the main
/// thread. Calling them from tokio workers (watcher loop, ksni tray thread,
/// async commands) corrupted the heap and segfaulted inside libgdk-3
/// (`malloc(): unaligned tcache chunk detected`).
fn on_main<F>(app: &AppHandle, f: F) -> Result<(), String>
where
    F: FnOnce(&AppHandle) -> Result<(), String> + Send + 'static,
{
    let app = app.clone();
    let main_app = app.clone();
    app.run_on_main_thread(move || {
        if let Err(e) = f(&main_app) {
            eprintln!("windows: main-thread window op failed: {e}");
        }
    })
    .map_err(|e| e.to_string())
}

pub fn position_tray_panel(app: &AppHandle) -> Result<(), String> {
    on_main(app, position_tray_panel_main)
}

fn position_tray_panel_main(app: &AppHandle) -> Result<(), String> {
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
        .or_else(|| app.available_monitors().ok().and_then(|mut m| m.pop()));

    if let Some(monitor) = monitor {
        let scale = monitor.scale_factor();
        let origin = monitor.position();
        let size = monitor.size();

        // Compute physical coordinates directly to avoid any DPI conversion inaccuracies
        let target_x =
            origin.x + (size.width as i32 - ((PANEL_W + OFFSET_RIGHT) * scale) as i32).max(0);
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
fn show_tray_panel_main(app: &AppHandle) -> Result<(), String> {
    let panel = app
        .get_webview_window(TRAY_PANEL_WINDOW)
        .ok_or_else(|| format!("Window '{TRAY_PANEL_WINDOW}' not found"))?;

    let _ = position_tray_panel_main(app);
    let _ = panel.unminimize();
    panel.show().map_err(|e| e.to_string())?;
    let _ = position_tray_panel_main(app);
    let _ = panel.set_focus();

    // On Linux window managers (GNOME/Mutter, XFCE, KWin), the WM asynchronously
    // applies its default window placement policy upon map. We re-apply position
    // after small intervals so the panel stays locked to the top-right. The
    // public wrapper marshals each call back onto the main thread.
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
    on_main(app, hide_tray_panel_main)
}

fn hide_tray_panel_main(app: &AppHandle) -> Result<(), String> {
    let panel = app
        .get_webview_window(TRAY_PANEL_WINDOW)
        .ok_or_else(|| format!("Window '{TRAY_PANEL_WINDOW}' not found"))?;

    panel.hide().map_err(|e| e.to_string())
}

pub fn toggle_tray_panel(app: &AppHandle) -> Result<(), String> {
    on_main(app, toggle_tray_panel_main)
}

fn toggle_tray_panel_main(app: &AppHandle) -> Result<(), String> {
    let panel = app
        .get_webview_window(TRAY_PANEL_WINDOW)
        .ok_or_else(|| format!("Window '{TRAY_PANEL_WINDOW}' not found"))?;

    let visible = panel.is_visible().map_err(|e| e.to_string())?;

    if visible {
        hide_tray_panel_main(app)
    } else {
        show_tray_panel_main(app)
    }
}

pub fn position_edge_portal(app: &AppHandle, side: Option<&str>) -> Result<(), String> {
    let side = side.map(str::to_string);
    on_main(app, move |app| {
        position_edge_portal_main(app, side.as_deref())
    })
}

fn position_edge_portal_main(app: &AppHandle, side: Option<&str>) -> Result<(), String> {
    let portal = app
        .get_webview_window(EDGE_PORTAL_WINDOW)
        .ok_or_else(|| format!("Window '{EDGE_PORTAL_WINDOW}' not found"))?;

    let side_str = if let Some(s) = side {
        s.to_string()
    } else {
        crate::runtime::get_layout(app)
            .map(|f| {
                match f.peer_side {
                    nexus_common::PeerSide::Left => "left",
                    nexus_common::PeerSide::Right => "right",
                    nexus_common::PeerSide::Top => "top",
                    nexus_common::PeerSide::Bottom => "bottom",
                }
                .to_string()
            })
            .unwrap_or_else(|_| "right".to_string())
    };

    let monitor = edge_monitor(app, &side_str).or_else(|| {
        portal
            .current_monitor()
            .ok()
            .flatten()
            .or_else(|| app.primary_monitor().ok().flatten())
            .or_else(|| portal.primary_monitor().ok().flatten())
            .or_else(|| app.available_monitors().ok().and_then(|mut m| m.pop()))
    });

    if let Some(monitor) = monitor {
        let origin = monitor.position();
        let size = monitor.size();

        let (target_x, target_y, target_w, target_h) = match side_str.as_str() {
            "left" => (origin.x, origin.y, 1u32, size.height),
            "top" => (origin.x, origin.y, size.width, 1u32),
            "bottom" => (
                origin.x,
                origin.y + (size.height as i32 - 1).max(0),
                size.width,
                1u32,
            ),
            _ => {
                let x = origin.x + (size.width as i32 - 1).max(0);
                (x, origin.y, 1u32, size.height)
            }
        };

        let _ = portal.set_min_size(Some(tauri::Size::Physical(tauri::PhysicalSize {
            width: 1,
            height: 1,
        })));
        let _ = portal.set_size(tauri::Size::Physical(tauri::PhysicalSize {
            width: target_w,
            height: target_h,
        }));
        let _ = portal.set_position(tauri::Position::Physical(tauri::PhysicalPosition {
            x: target_x,
            y: target_y,
        }));
        let _ = portal.set_always_on_top(true);

        #[cfg(target_os = "linux")]
        {
            use gtk::prelude::*;
            if let Ok(gtk_win) = portal.gtk_window() {
                let req_w = target_w as i32;
                let req_h = target_h as i32;
                gtk_win.set_size_request(req_w, req_h);
                for child in gtk_win.children() {
                    child.set_size_request(req_w, req_h);
                    if let Ok(container) = child.downcast::<gtk::Container>() {
                        for sub in container.children() {
                            sub.set_size_request(req_w, req_h);
                        }
                    }
                }
                gtk_win.resize(req_w, req_h);
            }
        }
    }

    Ok(())
}

fn edge_monitor(app: &AppHandle, side: &str) -> Option<tauri::Monitor> {
    let monitors = app.available_monitors().ok()?;
    if monitors.is_empty() {
        return None;
    }
    // Compare the far edge (origin + size), not the origin: with mixed monitor
    // sizes the origin alone can pick the wrong monitor for right/bottom.
    let right = |m: &tauri::Monitor| m.position().x + m.size().width as i32;
    let bottom = |m: &tauri::Monitor| m.position().y + m.size().height as i32;
    match side {
        "left" => monitors.into_iter().min_by_key(|m| m.position().x),
        "top" => monitors.into_iter().min_by_key(|m| m.position().y),
        "bottom" => monitors.into_iter().max_by_key(bottom),
        _ => monitors.into_iter().max_by_key(right),
    }
}

pub fn show_edge_portal(app: &AppHandle) -> Result<(), String> {
    on_main(app, show_edge_portal_main)
}

fn show_edge_portal_main(app: &AppHandle) -> Result<(), String> {
    let portal = app
        .get_webview_window(EDGE_PORTAL_WINDOW)
        .ok_or_else(|| format!("Window '{EDGE_PORTAL_WINDOW}' not found"))?;

    let _ = position_edge_portal_main(app, None);
    let _ = portal.show();
    let _ = portal.set_always_on_top(true);
    let _ = position_edge_portal_main(app, None);
    Ok(())
}

pub fn hide_edge_portal(app: &AppHandle) -> Result<(), String> {
    on_main(app, hide_edge_portal_main)
}

fn hide_edge_portal_main(app: &AppHandle) -> Result<(), String> {
    let portal = app
        .get_webview_window(EDGE_PORTAL_WINDOW)
        .ok_or_else(|| format!("Window '{EDGE_PORTAL_WINDOW}' not found"))?;

    portal.hide().map_err(|e| e.to_string())
}

pub fn open_main_window(app: &AppHandle) -> Result<(), String> {
    on_main(app, open_main_window_main)
}

fn open_main_window_main(app: &AppHandle) -> Result<(), String> {
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
    on_main(app, hide_main_window_main)
}

fn hide_main_window_main(app: &AppHandle) -> Result<(), String> {
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
        panel.on_window_event(move |event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                if let Some(p) = app_handle.get_webview_window(TRAY_PANEL_WINDOW) {
                    let _ = p.hide();
                }
            }
        });
    }

    Ok(())
}

pub fn configure_edge_portal(app: &mut tauri::App) -> tauri::Result<()> {
    if let Some(portal) = app.get_webview_window(EDGE_PORTAL_WINDOW) {
        let app_handle = app.handle().clone();
        portal.on_window_event(move |event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                if let Some(p) = app_handle.get_webview_window(EDGE_PORTAL_WINDOW) {
                    let _ = p.hide();
                }
            }
        });
    }

    Ok(())
}
