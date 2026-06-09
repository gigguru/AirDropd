//! Persistent AirDropd user settings.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

pub type SharedConfig = Arc<RwLock<AppConfig>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Name shown to nearby devices (default: Windows computer name).
    pub broadcast_name: String,
    /// Base folder for received files (files go in `{download_dir}/AirDropd/`).
    pub download_dir: PathBuf,
    /// When true, minimizing or closing hides to the system tray instead of exiting.
    pub minimize_to_tray: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            broadcast_name: default_broadcast_name(),
            download_dir: default_download_dir(),
            minimize_to_tray: true,
        }
    }
}

impl AppConfig {
    pub fn load() -> Self {
        match Self::load_from_disk() {
            Ok(cfg) => cfg,
            Err(e) => {
                tracing::warn!("Using default config ({})", e);
                Self::default()
            }
        }
    }

    pub fn load_from_disk() -> Result<Self> {
        let path = config_path();
        if !path.exists() {
            let cfg = Self::default();
            cfg.save()?;
            return Ok(cfg);
        }
        let text = std::fs::read_to_string(&path)?;
        let cfg: AppConfig = toml::from_str(&text).context("parse config.toml")?;
        Ok(cfg)
    }

    pub fn save(&self) -> Result<()> {
        let path = config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = toml::to_string_pretty(self)?;
        std::fs::write(path, text)?;
        Ok(())
    }

    /// Directory where incoming AirDrop files are stored (`…/AirDropd/`).
    pub fn receive_dir(&self) -> PathBuf {
        self.download_dir.join("AirDropd")
    }

    pub fn ensure_receive_dir(&self) -> Result<PathBuf> {
        let dir = self.receive_dir();
        std::fs::create_dir_all(&dir)?;
        Ok(dir)
    }
}

pub fn shared(config: AppConfig) -> SharedConfig {
    Arc::new(RwLock::new(config))
}

pub fn config_path() -> PathBuf {
    #[cfg(windows)]
    {
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            return PathBuf::from(local).join("AirDropd").join("config.toml");
        }
    }
    dirs_fallback_config()
}

fn dirs_fallback_config() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("config.toml")))
        .unwrap_or_else(|| PathBuf::from("config.toml"))
}

pub fn default_download_dir() -> PathBuf {
    #[cfg(windows)]
    {
        if let Ok(profile) = std::env::var("USERPROFILE") {
            return PathBuf::from(profile).join("Downloads");
        }
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

pub fn default_broadcast_name() -> String {
    hostname::get()
        .ok()
        .map(|h| h.to_string_lossy().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "AirDropd".to_string())
}

/// Pick a unique destination path inside the receive folder.
pub fn unique_receive_path(receive_dir: &Path, filename: &str) -> PathBuf {
    let safe: String = filename
        .chars()
        .map(|c| if r#"<>:"/\|?*"#.contains(c) { '_' } else { c })
        .collect();
    let base = receive_dir.join(if safe.is_empty() { "received_file" } else { &safe });
    if !base.exists() {
        return base;
    }
    let stem = base.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    let ext = base.extension().and_then(|s| s.to_str());
    for i in 1..1000 {
        let candidate = match ext {
            Some(e) => receive_dir.join(format!("{} ({}).{}", stem, i, e)),
            None => receive_dir.join(format!("{} ({})", stem, i)),
        };
        if !candidate.exists() {
            return candidate;
        }
    }
    receive_dir.join(format!("{}_{}", stem, uuid::Uuid::new_v4()))
}
