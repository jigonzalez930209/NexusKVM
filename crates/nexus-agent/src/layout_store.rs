use anyhow::Result;
use nexus_common::LayoutFile;
use std::path::{Path, PathBuf};

pub fn layout_path(data_dir: &Path) -> PathBuf {
    data_dir.join("layout.json")
}

pub fn load_or_default(data_dir: &Path) -> Result<LayoutFile> {
    let path = layout_path(data_dir);
    if path.is_file() {
        let raw = std::fs::read_to_string(&path)?;
        let file: LayoutFile = serde_json::from_str(&raw)?;
        file.layout.validate().map_err(anyhow::Error::msg)?;
        return Ok(file);
    }
    let file = LayoutFile::default_right(None);
    save(data_dir, &file)?;
    Ok(file)
}

pub fn save(data_dir: &Path, file: &LayoutFile) -> Result<()> {
    std::fs::create_dir_all(data_dir)?;
    file.layout.validate().map_err(anyhow::Error::msg)?;
    let path = layout_path(data_dir);
    std::fs::write(path, serde_json::to_string_pretty(file)?)?;
    Ok(())
}

pub fn agent_status_path(data_dir: &Path) -> PathBuf {
    data_dir.join("agent_status.json")
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentStatusFile {
    pub portal_available: bool,
    pub portal_error: Option<String>,
    pub peer_side: String,
    pub clipboard_ok: bool,
}

pub fn write_agent_status(data_dir: &Path, status: &AgentStatusFile) -> Result<()> {
    std::fs::create_dir_all(data_dir)?;
    std::fs::write(
        agent_status_path(data_dir),
        serde_json::to_string_pretty(status)?,
    )?;
    Ok(())
}
