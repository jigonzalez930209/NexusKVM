use std::fs;
use std::path::Path;

fn ensure_external_bins(target: &str) {
    let bins_dir = Path::new("binaries");
    let _ = fs::create_dir_all(bins_dir);
    let bin_names = ["nexus-kvmd", "nexus-agent", "nexusctl", "rkvm-client"];
    for name in &bin_names {
        let file_path = bins_dir.join(format!("{name}-{target}"));
        if !file_path.exists() {
            let _ = fs::write(&file_path, b"");
        }
    }
}

fn main() {
    let target = std::env::var("TARGET").unwrap_or_default();
    println!("cargo:rustc-env=TARGET_TRIPLE={target}");
    if !target.is_empty() {
        ensure_external_bins(&target);
    }
    tauri_build::build()
}
