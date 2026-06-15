//! Persistent AirDropd user settings.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

/// Who can discover this PC and which nearby devices appear on the radar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveryMode {
    /// Visible to everyone on the network; show phones, Macs, and reachable peers.
    Everyone,
    /// Visible with contacts-only receiver flags (best-effort without Apple signing).
    ContactsOnly,
    /// Stop advertising — this PC does not appear in others' AirDrop sheets.
    ReceivingOff,
    /// Radar shows Apple phones, tablets, and Macs only.
    AppleDevices,
    /// Radar shows Android BLE peers only.
    AndroidDevices,
    /// Radar shows AirTags and Find My beacons (live RSSI for locating).
    AirTags,
}

impl Default for DiscoveryMode {
    fn default() -> Self {
        Self::Everyone
    }
}

impl DiscoveryMode {
    pub fn discoverable(&self) -> bool {
        !matches!(self, Self::ReceivingOff)
    }

    pub fn contacts_only(&self) -> bool {
        matches!(self, Self::ContactsOnly)
    }

    pub fn device_filter(&self) -> DeviceFilter {
        match self {
            Self::AppleDevices => DeviceFilter::Apple,
            Self::AndroidDevices => DeviceFilter::Android,
            Self::AirTags => DeviceFilter::AirTags,
            _ => DeviceFilter::All,
        }
    }

    /// Whether BLE accessories (AirPods, AirTags, …) should be collected at all.
    pub fn include_accessories(&self, show_all_devices: bool) -> bool {
        matches!(self, Self::AirTags) || show_all_devices
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Everyone => "Everyone",
            Self::ContactsOnly => "Contacts Only",
            Self::ReceivingOff => "No One",
            Self::AppleDevices => "Peer Devices Only",
            Self::AndroidDevices => "Android Only",
            Self::AirTags => "Trackers Only",
        }
    }
}

impl std::fmt::Display for DiscoveryMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

/// Filters applied to the nearby-device radar list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DeviceFilter {
    #[default]
    All,
    Apple,
    Android,
    AirTags,
}

pub type SharedConfig = Arc<RwLock<AppConfig>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Name shown to nearby devices (default: Windows computer name).
    pub broadcast_name: String,
    /// Base folder for received files (files go in `{download_dir}/AirDropd/`).
    pub download_dir: PathBuf,
    /// When true, minimizing or closing hides to the system tray instead of exiting.
    pub minimize_to_tray: bool,
    /// Stable 12-char hex identity hash (`ph` in Apple mDNS TXT records).
    #[serde(default = "AppConfig::generate_device_ph")]
    pub device_ph: String,
    /// Stable device identifier for companion-link records.
    #[serde(default = "AppConfig::generate_device_id")]
    pub device_id: String,
    /// Random 12-hex mDNS service instance id (OpenDrop-style).
    #[serde(default = "AppConfig::generate_service_id")]
    pub service_id: String,
    /// Whether this PC advertises itself for incoming AirDrop transfers.
    #[serde(default = "default_discoverable")]
    pub discoverable: bool,
    /// Contacts-only visibility (requires Apple-signed identity for full compatibility).
    #[serde(default)]
    pub contacts_only: bool,
    /// User already added Windows Firewall exceptions (skip future prompts).
    #[serde(default)]
    pub firewall_exceptions_added: bool,
    /// User dismissed the firewall prompt without adding rules.
    #[serde(default)]
    pub firewall_prompt_dismissed: bool,
    /// Automatically accept incoming AirDrop transfers without prompting.
    #[serde(default)]
    pub auto_accept_incoming: bool,
    /// Discovery visibility and nearby-device filter (main-view picker).
    #[serde(default)]
    pub discovery_mode: DiscoveryMode,
    /// Show every nearby Apple device on the radar, including accessories
    /// (AirPods, AirTags, Apple Watch) — useful for locating a lost device.
    #[serde(default)]
    pub show_all_devices: bool,
    /// Web Drop guest device id → folder name (same phone always lands in the
    /// same subfolder under `WebDrop/`).
    #[serde(default)]
    pub webdrop_devices: HashMap<String, String>,
    /// Demo usage counters and product-key registration.
    #[serde(default)]
    pub license: crate::licensing::LicenseFields,
}

fn default_discoverable() -> bool {
    true
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            broadcast_name: default_broadcast_name(),
            download_dir: default_download_dir(),
            minimize_to_tray: true,
            device_ph: Self::generate_device_ph(),
            device_id: Self::generate_device_id(),
            service_id: Self::generate_service_id(),
            discoverable: true,
            contacts_only: false,
            firewall_exceptions_added: false,
            firewall_prompt_dismissed: false,
            auto_accept_incoming: false,
            discovery_mode: DiscoveryMode::Everyone,
            show_all_devices: false,
            webdrop_devices: HashMap::new(),
            license: crate::licensing::LicenseFields::default(),
        }
    }
}

impl AppConfig {
    pub fn license_store(&mut self) -> crate::licensing::LicenseStore<'_> {
        crate::licensing::LicenseStore::new(&mut self.license)
    }

    pub fn license_status(&self) -> crate::licensing::LicenseStatus {
        let mut fields = self.license.clone();
        crate::licensing::LicenseStore::new(&mut fields).status()
    }
    pub fn generate_device_ph() -> String {
        let seed = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "AirDropd".to_string());
        let mut hasher = Sha256::new();
        hasher.update(seed.as_bytes());
        hasher.update(b"AirDropd-ph-v1");
        hex::encode(&hasher.finalize()[..6])
    }

    pub fn generate_device_id() -> String {
        uuid::Uuid::new_v4().simple().to_string().to_uppercase()
    }

    pub fn generate_service_id() -> String {
        format!("{:012x}", rand::random::<u64>() & 0xFFFFFFFFFFFF)
    }

    pub fn device_ph_bytes(&self) -> [u8; 6] {
        let mut out = [0u8; 6];
        let bytes = hex::decode(&self.device_ph).unwrap_or_else(|_| {
            hex::decode(Self::generate_device_ph()).unwrap_or_default()
        });
        for (i, b) in bytes.into_iter().take(6).enumerate() {
            out[i] = b;
        }
        out
    }

    pub fn ensure_identity(&mut self) {
        if self.device_ph.len() != 12 {
            self.device_ph = Self::generate_device_ph();
        }
        if self.device_id.is_empty() {
            self.device_id = Self::generate_device_id();
        }
        if self.service_id.len() != 12 {
            self.service_id = Self::generate_service_id();
        }
        if self.broadcast_name.trim().is_empty() {
            self.broadcast_name = default_broadcast_name();
        }
    }

    pub fn load() -> Self {
        match Self::load_from_disk() {
            Ok(mut cfg) => {
                cfg.ensure_identity();
                cfg.migrate_discovery_mode();
                cfg
            }
            Err(e) => {
                tracing::warn!("Using default config ({})", e);
                Self::default()
            }
        }
    }

    pub fn load_from_disk() -> Result<Self> {
        let path = config_path();
        if !path.exists() {
            let mut cfg = Self::default();
            cfg.ensure_identity();
            cfg.save()?;
            return Ok(cfg);
        }
        let text = std::fs::read_to_string(&path)?;
        let mut cfg: AppConfig = toml::from_str(&text).context("parse config.toml")?;
        cfg.ensure_identity();
        cfg.migrate_discovery_mode();
        Ok(cfg)
    }

    /// Keep legacy `discoverable` / `contacts_only` flags aligned with `discovery_mode`.
    pub fn migrate_discovery_mode(&mut self) {
        if !self.discoverable && self.discovery_mode == DiscoveryMode::Everyone {
            self.discovery_mode = DiscoveryMode::ReceivingOff;
        } else if self.contacts_only && self.discovery_mode == DiscoveryMode::Everyone {
            self.discovery_mode = DiscoveryMode::ContactsOnly;
        }
        self.sync_discovery_flags();
    }

    pub fn set_discovery_mode(&mut self, mode: DiscoveryMode) {
        self.discovery_mode = mode;
        self.sync_discovery_flags();
    }

    pub fn sync_discovery_flags(&mut self) {
        self.discoverable = self.discovery_mode.discoverable();
        self.contacts_only = self.discovery_mode.contacts_only();
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

    pub fn receive_dir(&self) -> PathBuf {
        self.download_dir.join("AirDropd")
    }

    /// QR / Web Drop uploads: `{download_dir}/AirDropd/WebDrop/`.
    pub fn webdrop_dir(&self) -> PathBuf {
        self.receive_dir().join("WebDrop")
    }

    /// Example default layout shown in Settings.
    pub fn default_save_paths_hint() -> String {
        let base = default_download_dir();
        format!(
            "{}/AirDropd/  ·  {}/AirDropd/WebDrop/",
            base.display(),
            base.display()
        )
    }

    pub fn ensure_receive_dir(&self) -> Result<PathBuf> {
        let dir = self.receive_dir();
        std::fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    /// Folder for a Web Drop guest device. The same `guest_id` (stored in the
    /// phone browser) always maps to the same folder, so repeat uploads before
    /// a show accumulate in one place instead of scattering.
    pub fn resolve_webdrop_folder(
        &mut self,
        guest_id: &str,
        guest_label: &str,
        client_ip: &str,
    ) -> Result<PathBuf> {
        let webdrop_root = self.ensure_receive_dir()?.join("WebDrop");
        std::fs::create_dir_all(&webdrop_root)?;

        let key = if guest_id.trim().is_empty() {
            format!("ip:{}", client_ip)
        } else {
            guest_id.trim().to_string()
        };

        if !self.webdrop_devices.contains_key(&key) {
            let folder = if !guest_label.trim().is_empty() {
                sanitize_folder_name(guest_label.trim())
            } else if !guest_id.trim().is_empty() {
                let short: String = guest_id.chars().filter(|c| c.is_ascii_hexdigit()).take(8).collect();
                format!("Guest-{}", if short.is_empty() { "device" } else { &short })
            } else {
                format!("Guest-{}", client_ip.replace([':', '.'], "-"))
            };
            self.webdrop_devices.insert(key.clone(), folder);
            let _ = self.save();
        }

        let name = self
            .webdrop_devices
            .get(&key)
            .cloned()
            .unwrap_or_else(|| "Guest".to_string());
        let dir = webdrop_root.join(&name);
        std::fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    /// Rename a guest folder under WebDrop and update the device-id mapping.
    pub fn rename_webdrop_folder(&mut self, old_folder: &str, new_label: &str) -> Result<PathBuf> {
        let new_folder = sanitize_folder_name(new_label);
        if new_folder.is_empty() {
            anyhow::bail!("invalid folder name");
        }
        let root = self.webdrop_dir();
        let old_path = root.join(old_folder);
        if !old_path.is_dir() {
            anyhow::bail!("folder not found: {old_folder}");
        }
        let new_path = root.join(&new_folder);
        if new_path.exists() && old_path != new_path {
            anyhow::bail!("a folder named \"{new_folder}\" already exists");
        }
        if old_path != new_path {
            std::fs::rename(&old_path, &new_path)?;
        }
        for folder in self.webdrop_devices.values_mut() {
            if folder == old_folder {
                *folder = new_folder.clone();
            }
        }
        self.save()?;
        Ok(new_path)
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
    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("AirDropd")
                .join("config.toml");
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
    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join("Downloads");
        }
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// Turn a guest-supplied label into a safe folder name.
pub fn sanitize_folder_name(name: &str) -> String {
    let mut out: String = name
        .chars()
        .map(|c| if r#"<>:"/\|?*"#.contains(c) { '_' } else { c })
        .collect();
    out = out.trim().trim_matches('.').to_string();
    if out.is_empty() {
        return "Guest".to_string();
    }
    if out.len() > 48 {
        out.truncate(48);
        out = out.trim_end().to_string();
    }
    out
}

pub fn default_broadcast_name() -> String {
    hostname::get()
        .ok()
        .map(|h| h.to_string_lossy().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "AirDropd".to_string())
}

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
