//! Linux system tray built directly on the freedesktop StatusNotifierItem
//! spec (ksni) instead of tauri's libappindicator backend.
//!
//! Why: libappindicator hardcodes `ItemIsMenu = true`, so GNOME's
//! AppIndicator extension opens the menu on left click and NEVER delivers
//! click events to the app (tauri docs: "Linux: Unsupported"). With ksni we
//! own the SNI registration, so `Activate` (left click) reaches us and we can
//! toggle the quick panel, plus we get the pointer coordinates to place it.

use crate::runtime::AppRuntime;
use crate::window_labels::MAIN_TRAY_ID;
use crate::{tray, windows};
use ksni::blocking::TrayMethods;
use ksni::menu::{MenuItem, StandardItem};
use ksni::{Icon, ToolTip, Tray};
use std::sync::Arc;
use tauri::AppHandle;

pub fn create_tray(app: &mut tauri::App, rt: Arc<AppRuntime>) -> tauri::Result<()> {
    let handle = app.handle().clone();
    std::thread::Builder::new()
        .name(format!("{MAIN_TRAY_ID}-ksni"))
        .spawn(move || {
            let tray_service = LinuxTray { app: handle, rt };
            if let Err(e) = tray_service.spawn() {
                eprintln!("Failed to register StatusNotifierItem tray: {e}");
            }
        })
        .map_err(|e| tauri::Error::Anyhow(anyhow::anyhow!("tray thread: {e}")))?;
    Ok(())
}

pub struct LinuxTray {
    pub app: AppHandle,
    pub rt: Arc<AppRuntime>,
}

impl Tray for LinuxTray {
    const MENU_ON_ACTIVATE: bool = false;

    fn id(&self) -> String {
        MAIN_TRAY_ID.to_string()
    }

    fn title(&self) -> String {
        "NexusKVM".into()
    }

    fn category(&self) -> ksni::Category {
        ksni::Category::ApplicationStatus
    }

    fn status(&self) -> ksni::Status {
        ksni::Status::Active
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            icon_name: "nexuskvm".into(),
            icon_pixmap: tray_icon_pixmap(),
            title: "NexusKVM — Spatial KVM".into(),
            description: "Left click: quick panel\nRight click: menu".into(),
        }
    }

    fn icon_pixmap(&self) -> Vec<Icon> {
        tray_icon_pixmap()
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        let _ = windows::toggle_tray_panel(&self.app);
    }

    fn secondary_activate(&mut self, _x: i32, _y: i32) {
        let _ = windows::open_main_window(&self.app);
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            StandardItem {
                label: "Open Quick Panel".into(),
                icon_name: "view-grid".into(),
                activate: Box::new(|t: &mut Self| {
                    let _ = windows::toggle_tray_panel(&t.app);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Open Main Window".into(),
                icon_name: "video-display".into(),
                activate: Box::new(|t: &mut Self| {
                    let _ = windows::open_main_window(&t.app);
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Quit NexusKVM".into(),
                icon_name: "application-exit".into(),
                activate: Box::new(|t: &mut Self| {
                    let app = t.app.clone();
                    let rt = t.rt.clone();
                    // quit_app shows a modal dialog; keep the dbus thread free.
                    std::thread::spawn(move || {
                        tray::quit_app(&app, &rt);
                    });
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

/// Decode the bundled 32x32 icon into SNI ARGB32 (network byte order).
fn tray_icon_pixmap() -> Vec<Icon> {
    let img = tauri::image::Image::from_bytes(include_bytes!("../icons/32x32.png")).ok();
    let Some(img) = img else {
        return Vec::new();
    };
    let (w, h) = (img.width() as i32, img.height() as i32);
    let rgba = img.rgba();
    let mut argb = Vec::with_capacity(rgba.len());
    for px in rgba.chunks_exact(4) {
        argb.push(px[3]); // A
        argb.push(px[0]); // R
        argb.push(px[1]); // G
        argb.push(px[2]); // B
    }
    vec![Icon {
        width: w,
        height: h,
        data: argb,
    }]
}
