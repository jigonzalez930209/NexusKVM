use std::sync::atomic::AtomicBool;

pub struct AppLifecycleState {
    pub quitting: AtomicBool,
}

impl Default for AppLifecycleState {
    fn default() -> Self {
        Self {
            quitting: AtomicBool::new(false),
        }
    }
}
