fn main() {
    #[cfg(target_os = "linux")]
    {
        // Force x11 backend so GTK/WebKitGTK supports programmatic window positioning
        // (xdg_shell in native Wayland blocks apps from setting absolute window coordinates).
        std::env::set_var("GDK_BACKEND", "x11");
    }

    nexuskvm_ui_lib::run()
}
