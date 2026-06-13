//! Main Iced user-interface module.
//!
//! This module contains the complete Iced UI implementation with a modern,
//! responsive design.

use iced::{
    executor,
    event,
    window,
    Application, Command, Element, Event, Settings, Subscription, Theme as IcedTheme,
    time,
};

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

// UI modules.
pub mod components;
pub mod assets;
pub mod device_icons;
pub mod distance;
pub mod icons;
pub mod messages;
pub mod qr;
pub mod radar;
pub mod styles;
pub mod tray;
pub mod views;
pub mod widgets;

// Re-export main message type.
pub use messages::Message;

/// How long a /Discover probe result stays valid before re-probing a device.
const PROBE_CACHE_TTL: Duration = Duration::from_secs(300);

type ProbeNameCache =
    std::sync::Mutex<HashMap<String, (std::time::Instant, Option<String>)>>;

/// Cache of /Discover probe results keyed by `ip:port`.
fn probe_name_cache() -> &'static ProbeNameCache {
    static CACHE: std::sync::OnceLock<ProbeNameCache> = std::sync::OnceLock::new();
    CACHE.get_or_init(|| std::sync::Mutex::new(HashMap::new()))
}

/// Stable 4-hex-char suffix so anonymous Apple BLE devices stay distinct on
/// the radar. (Apple rotates BLE addresses every ~15 minutes by design, so
/// the suffix changes over time — that is inherent to the privacy scheme.)
fn short_ble_suffix(id: &str) -> String {
    let mut hash: u16 = 0;
    for b in id.bytes() {
        hash = hash.rotate_left(5) ^ (b as u16);
    }
    format!("{:04X}", hash)
}

fn anonymous_mobile_ble_name(
    profile: &crate::network::discovery::BleMobileProfile,
    id: &str,
) -> String {
    let suffix = short_ble_suffix(id);
    use crate::network::discovery::BleMobilePlatform;
    match profile.platform {
        Some(BleMobilePlatform::Apple) => match profile.device_class {
            "tablet" => format!("iPad nearby {suffix}"),
            _ => format!("iPhone nearby {suffix}"),
        },
        Some(BleMobilePlatform::Android) => match profile.device_class {
            "tablet" => format!("Android tablet {suffix}"),
            "watch" => format!("Android watch {suffix}"),
            _ => format!("Android phone {suffix}"),
        },
        None => format!("Mobile phone {suffix}"),
    }
}
 
/// Application theme, used by `styles` for custom styling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Theme {
    #[default]
    Dark,
    Light,
}
 
/// Main AirDropd application state.
pub struct AirDropdApp {
    /// Background services (mDNS, BLE, AirDrop server)
    services: Arc<crate::AirDropdServices>,

    /// Current application view
    current_view: AppView,
    
    /// Discovered network devices
    discovered_devices: Vec<crate::network::DiscoveredDevice>,
    
    /// Currently selected device
    selected_device: Option<crate::network::DiscoveredDevice>,
    
    /// Scanning state
    is_scanning: bool,
    
    /// AirDrop status
    airdrop_status: crate::protocols::airdrop::AirDropStatus,
    
    /// File-transfer progress (0.0-100.0)
    file_transfer_progress: Option<f32>,
    
    /// Active notifications
    notifications: Vec<messages::NotificationMessage>,
    
    /// Current theme
    theme: Theme,
    
    /// Discovery visibility (macOS AirDrop-style)
    discovery_visibility: views::settings_view::AirDropVisibility,
    
    /// Persisted settings view to avoid lifetime issues
    settings_view: views::settings_view::SettingsView,
    
    /// Link-send dialog state
    show_link_dialog: bool,

    /// About dialog overlay
    show_about: bool,
    
    /// URL to send as a link
    link_url: String,
    
    /// General loading state
    is_loading: bool,
    
    /// Status message
    status_message: String,

    /// Main window hidden in system tray
    window_hidden: bool,
    /// System tray/menu-bar icon initialized (deferred until GUI loop is up).
    tray_initialized: bool,

    /// Subscription to incoming file notifications
    received_rx: Option<tokio::sync::broadcast::Receiver<std::path::PathBuf>>,

    /// Splash animation state
    splash_frames: assets::SplashFrames,
    splash_tick: usize,

    /// Animated sonar pulse while scanning
    sonar_tick: u32,

    /// Pending incoming AirDrop /Ask request (shown in accept dialog)
    pending_incoming: Option<crate::protocols::incoming_transfer::IncomingTransferDetails>,

    /// OS file-drag currently hovering over the window
    drop_hover: bool,

    /// Files collected from the current drop gesture (debounced)
    dropped_files: Vec<std::path::PathBuf>,
    drop_generation: u64,

    /// Dropped files waiting for the user to pick a recipient
    pending_recipient_files: Option<Vec<std::path::PathBuf>>,

    /// Cached Web Drop URL + QR image (recomputed when opening the screen)
    web_drop_url: Option<String>,
    web_drop_qr: Option<iced::widget::image::Handle>,
    web_drop_listening: bool,
    /// Large QR for DJ mode (separate cache so toggling views stays fast)
    dj_qr: Option<iced::widget::image::Handle>,

    /// Files received during the current DJ mode session
    dj_files_received: u32,
    dj_last_file: Option<String>,
    /// Restore AirDrop auto-accept when leaving DJ mode
    dj_saved_auto_accept: Option<bool>,
    /// Guest upload folders shown as file-cabinet drawers
    dj_drawers: Vec<views::dj_mode_view::DjDrawer>,
    dj_drawer_order: Vec<String>,
    dj_renaming_folder: Option<String>,
    dj_rename_text: String,

    /// Sonar vs list layout on the main screen
    device_view_mode: views::device_list_view::DeviceViewMode,

    /// Active list sort column and direction
    list_sort_column: views::device_list_view::ListSortColumn,
    list_sort_ascending: bool,

    /// When true, discovery refreshes and sonar sweep are paused (event mode).
    discovery_frozen: bool,
}

/// Available application views.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppView {
    /// Main view with nearby devices and action panel
    Main,
    /// Live Activity protocol-event feed
    Activity,
    /// QR-based Web Drop receive screen
    WebDrop,
    /// Full-screen DJ receive mode (large QR + auto-save)
    DjMode,
    /// Settings view
    Settings,
    /// Initial loading view
    Loading,
    /// Startup splash with branded icon animation
    Splash,
}

impl Default for AppView {
    fn default() -> Self {
        Self::Loading
    }
}

impl Application for AirDropdApp {
    type Message = Message;
    type Theme = IcedTheme;
    type Executor = executor::Default;
    type Flags = Arc<crate::AirDropdServices>;

    fn new(services: Self::Flags) -> (Self, Command<Self::Message>) {
        let cfg = services
            .config
            .read()
            .map(|c| c.clone())
            .unwrap_or_default();
        let settings_view = views::settings_view::SettingsView::from_config(&cfg);
        let received_rx = services.received_tx.subscribe();

        let app = Self {
            services,
            current_view: AppView::Splash,
            status_message: "Starting...".to_string(),
            is_loading: true,
            theme: Theme::default(),
            discovery_visibility: cfg.discovery_mode,
            settings_view,
            show_about: false,
            discovered_devices: Vec::new(),
            selected_device: None,
            is_scanning: false,
            airdrop_status: crate::protocols::airdrop::AirDropStatus::Idle,
            file_transfer_progress: None,
            notifications: Vec::new(),
            show_link_dialog: false,
            link_url: String::new(),
            window_hidden: false,
            tray_initialized: false,
            received_rx: Some(received_rx),
            splash_frames: assets::SplashFrames::new(),
            splash_tick: 0,
            sonar_tick: 0,
            pending_incoming: None,
            drop_hover: false,
            dropped_files: Vec::new(),
            drop_generation: 0,
            pending_recipient_files: None,
            web_drop_url: None,
            web_drop_qr: None,
            web_drop_listening: false,
            dj_qr: None,
            dj_files_received: 0,
            dj_last_file: None,
            dj_saved_auto_accept: None,
            dj_drawers: Vec::new(),
            dj_drawer_order: Vec::new(),
            dj_renaming_folder: None,
            dj_rename_text: String::new(),
            device_view_mode: views::device_list_view::DeviceViewMode::Sonar,
            list_sort_column: views::device_list_view::ListSortColumn::Distance,
            list_sort_ascending: true,
            discovery_frozen: false,
        };

        (app, Command::none())
    }

    fn title(&self) -> String {
        match self.current_view {
            AppView::Main => "AirDropd".to_string(),
            AppView::Activity => "AirDropd — Live Activity".to_string(),
            AppView::WebDrop => "AirDropd — Receive via QR".to_string(),
            AppView::DjMode => "AirDropd — DJ Mode".to_string(),
            AppView::Settings => "AirDropd — Settings".to_string(),
            AppView::Loading | AppView::Splash => "AirDropd".to_string(),
        }
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        match message {
            Message::SplashTick => {
                if self.current_view != AppView::Splash {
                    return Command::none();
                }
                self.splash_tick = self.splash_tick.saturating_add(1);
                if self.splash_tick >= assets::SPLASH_TOTAL_TICKS {
                    return Command::perform(async {}, |_| Message::SplashComplete);
                }
                Command::none()
            }

            Message::SplashComplete => {
                self.current_view = AppView::Main;
                self.is_loading = false;
                self.status_message = "Ready".to_string();
                if !self.tray_initialized {
                    let name = self
                        .services
                        .config
                        .read()
                        .map(|c| c.broadcast_name.clone())
                        .unwrap_or_else(|_| crate::config::default_broadcast_name());
                    if tray::init_tray(&format!("AirDropd — {}", name)).is_ok() {
                        self.tray_initialized = true;
                    }
                }
                Command::perform(async {}, |_| Message::CheckFirewall)
            }

            Message::SonarTick => {
                if !self.discovery_frozen {
                    self.sonar_tick = self.sonar_tick.wrapping_add(1);
                }
                Command::none()
            }

            Message::CheckFirewall => {
                let skip = self
                    .services
                    .config
                    .read()
                    .map(|c| c.firewall_exceptions_added || c.firewall_prompt_dismissed)
                    .unwrap_or(false);

                if skip {
                    return Command::perform(async {}, |_| Message::StartScanning);
                }

                Command::perform(
                    async {
                        tokio::task::spawn_blocking(|| {
                            let audit = crate::network::firewall::audit();
                            crate::network::firewall::prompt_and_fix(&audit)
                        })
                        .await
                        .unwrap_or(crate::network::firewall::FirewallPromptResult::Failed(
                            "Firewall check failed".to_string(),
                        ))
                    },
                    Message::FirewallPromptComplete,
                )
            }

            Message::FirewallPromptComplete(result) => {
                match &result {
                    crate::network::firewall::FirewallPromptResult::Added => {
                        if let Ok(mut cfg) = self.services.config.write() {
                            cfg.firewall_exceptions_added = true;
                            cfg.firewall_prompt_dismissed = false;
                            let _ = cfg.save();
                        }
                        self.add_notification(
                            "Firewall updated".to_string(),
                            "Windows Firewall exceptions were added for AirDropd.".to_string(),
                            messages::NotificationType::Success,
                        );
                    }
                    crate::network::firewall::FirewallPromptResult::Declined => {
                        if let Ok(mut cfg) = self.services.config.write() {
                            cfg.firewall_prompt_dismissed = true;
                            let _ = cfg.save();
                        }
                        self.add_notification(
                            "Firewall access".to_string(),
                            "AirDropd may not work correctly until required ports are allowed in Windows Firewall.".to_string(),
                            messages::NotificationType::Warning,
                        );
                    }
                    crate::network::firewall::FirewallPromptResult::Failed(err) => {
                        let ports = crate::network::firewall::format_port_list(
                            crate::network::firewall::AIRDROP_PORTS,
                        );
                        self.add_notification(
                            "Could not update firewall".to_string(),
                            format!(
                                "Try running AirDropd as Administrator, or allow these ports manually:\n{ports}\n\n{err}"
                            ),
                            messages::NotificationType::Warning,
                        );
                    }
                    crate::network::firewall::FirewallPromptResult::NotNeeded => {}
                }

                Command::perform(async {}, |_| Message::StartScanning)
            }

            Message::InitializationComplete => {
                self.current_view = AppView::Main;
                self.is_loading = false;
                self.status_message = "Ready".to_string();
                
                // Start automatic scanning.
                Command::perform(
                    async { () },
                    |_| Message::StartScanning,
                )
            }

            Message::StartScanning => {
                self.is_scanning = true;
                self.sonar_tick = 0;
                self.status_message = "Scanning for devices...".to_string();
                
                let services = self.services.clone();
                Command::perform(
                    Self::fetch_devices(services),
                    Message::DevicesUpdated,
                )
            }

            Message::RefreshDevices => {
                let services = self.services.clone();
                Command::perform(
                    Self::fetch_devices(services),
                    Message::DevicesRefreshed,
                )
            }

            Message::DevicesRefreshed(devices) => {
                if self.discovery_frozen {
                    return Command::none();
                }
                self.discovered_devices = devices;
                self.sync_selected_device();
                if !self.is_scanning {
                    self.status_message = format!(
                        "Found {} devices",
                        self.discovered_devices.len()
                    );
                }
                Command::none()
            }

            Message::StopScanning => {
                self.is_scanning = false;
                self.status_message = "Scan stopped".to_string();
                Command::none()
            }

            Message::DevicesUpdated(devices) => {
                if self.discovery_frozen {
                    self.is_scanning = false;
                    return Command::none();
                }
                self.discovered_devices = devices;
                self.sync_selected_device();
                self.is_scanning = false;
                self.status_message = format!(
                    "Found {} devices",
                    self.discovered_devices.len()
                );
                Command::none()
            }

            Message::DeviceSelected(device) => {
                self.selected_device = Some(device.clone());
                self.status_message = format!("Selected: {}", device.display_title());
                Command::none()
            }

            Message::DeviceDeselected => {
                if self.file_transfer_progress.is_none() {
                    self.selected_device = None;
                }
                Command::none()
            }

            Message::SetDeviceViewMode(mode) => {
                self.device_view_mode = mode;
                Command::none()
            }

            Message::ListSortBy(column) => {
                if self.list_sort_column == column {
                    self.list_sort_ascending = !self.list_sort_ascending;
                } else {
                    self.list_sort_column = column;
                    self.list_sort_ascending = true;
                }
                Command::none()
            }

            Message::ToggleDiscoveryFreeze => {
                self.discovery_frozen = !self.discovery_frozen;
                Command::none()
            }

            Message::SendFile(device) => {
                let device = device.clone();
                Command::perform(
                    async move {
                        let files = rfd::AsyncFileDialog::new()
                            .set_title("Choose files to send")
                            .pick_files()
                            .await;
                        let paths: Vec<std::path::PathBuf> = files
                            .map(|handles| {
                                handles.iter().map(|h| h.path().to_path_buf()).collect()
                            })
                            .unwrap_or_default();
                        (device, paths)
                    },
                    |(device, paths)| {
                        if paths.is_empty() {
                            Message::Tick
                        } else {
                            Message::ChooseRecipientWithFiles(device, paths)
                        }
                    },
                )
            }

            Message::SendFolder(device) => {
                let device = device.clone();
                Command::perform(
                    async move {
                        let folder = rfd::AsyncFileDialog::new()
                            .set_title("Choose a folder to send")
                            .pick_folder()
                            .await;
                        let paths: Vec<std::path::PathBuf> = folder
                            .map(|h| vec![h.path().to_path_buf()])
                            .unwrap_or_default();
                        (device, paths)
                    },
                    |(device, paths)| {
                        if paths.is_empty() {
                            Message::Tick
                        } else {
                            Message::ChooseRecipientWithFiles(device, paths)
                        }
                    },
                )
            }

            Message::ChooseRecipientWithFiles(device, paths) => self.start_send(device, paths),

            Message::SendLink(device, url) => {
                self.show_link_dialog = false;
                let url = url.trim().to_string();
                let url = if url.starts_with("http://") || url.starts_with("https://") {
                    url
                } else {
                    format!("https://{}", url)
                };
                self.link_url.clear();
                if device.address.is_unspecified() {
                    self.add_notification(
                        "Cannot send link".to_string(),
                        format!(
                            "{} is visible via Bluetooth only — wait for Wi‑Fi discovery.",
                            device.name
                        ),
                        messages::NotificationType::Error,
                    );
                    return Command::none();
                }
                self.add_notification(
                    "Sending link".to_string(),
                    format!("Sending link to {}", device.name),
                    messages::NotificationType::Info,
                );
                self.airdrop_status = crate::protocols::airdrop::AirDropStatus::Connecting;
                let service_id = self
                    .services
                    .config
                    .read()
                    .map(|c| c.service_id.clone())
                    .unwrap_or_default();
                let port = if device.port > 0 { device.port } else { 8770 };
                let addr = std::net::SocketAddr::new(device.address, port);
                Command::perform(
                    async move {
                        crate::protocols::airdrop_client::AirDropClient::send_link(
                            addr,
                            &url,
                            &service_id,
                        )
                        .await
                        .map_err(|e| e.to_string())
                    },
                    |res| Message::FileSendCompleted(res),
                )
            }

            Message::FileSendProgress(progress) => {
                self.file_transfer_progress = Some(progress);
                self.airdrop_status = crate::protocols::airdrop::AirDropStatus::Transferring(progress);
                Command::none()
            }

            Message::FileSendCompleted(result) => {
                self.file_transfer_progress = None;
                self.airdrop_status = crate::protocols::airdrop::AirDropStatus::Idle;
                match result {
                    Ok(()) => {
                        self.selected_device = None;
                        self.add_notification(
                            "Transfer complete".to_string(),
                            "Operation completed successfully".to_string(),
                            messages::NotificationType::Success,
                        );
                    }
                    Err(e) => self.add_notification(
                        "Transfer failed".to_string(),
                        e,
                        messages::NotificationType::Error,
                    ),
                }
                Command::none()
            }

            Message::ShowLinkDialog => {
                self.show_link_dialog = true;
                Command::none()
            }

            Message::HideLinkDialog => {
                self.show_link_dialog = false;
                self.link_url.clear();
                Command::none()
            }

            Message::LinkInputChanged(url) => {
                self.link_url = url;
                Command::none()
            }

            Message::ShowNotification(notification) => {
                self.notifications.push(notification);
                
                // Automatically remove the notification after 5 seconds.
                Command::perform(
                    async {
                        tokio::time::sleep(Duration::from_secs(5)).await;
                    },
                    |_| Message::HideNotification,
                )
            }

            Message::HideNotification => {
                if !self.notifications.is_empty() {
                    self.notifications.remove(0);
                }
                Command::none()
            }

            Message::FilesHoveringWindow(hovering) => {
                self.drop_hover = hovering;
                Command::none()
            }

            Message::FileDroppedOnWindow(path) => {
                // The OS delivers one event per dropped file; debounce so a
                // multi-file drop becomes a single transfer.
                self.drop_hover = false;
                self.dropped_files.push(path);
                self.drop_generation = self.drop_generation.wrapping_add(1);
                let generation = self.drop_generation;
                Command::perform(
                    async move {
                        tokio::time::sleep(Duration::from_millis(300)).await;
                        generation
                    },
                    Message::ProcessDroppedFiles,
                )
            }

            Message::ProcessDroppedFiles(generation) => {
                if generation != self.drop_generation || self.dropped_files.is_empty() {
                    return Command::none();
                }
                let files = std::mem::take(&mut self.dropped_files);

                // Send straight away when the recipient is unambiguous.
                if let Some(device) = self.selected_device.clone() {
                    return self.start_send(device, files);
                }
                let reachable: Vec<_> = self
                    .discovered_devices
                    .iter()
                    .filter(|d| !d.address.is_unspecified() && d.port > 0)
                    .cloned()
                    .collect();
                if reachable.len() == 1 {
                    return self.start_send(reachable[0].clone(), files);
                }
                if self.discovered_devices.is_empty() {
                    self.add_notification(
                        "No devices nearby".to_string(),
                        "Open AirDrop on the receiving device, then drop the files again."
                            .to_string(),
                        messages::NotificationType::Warning,
                    );
                    return Command::none();
                }
                // Several candidates: let the user choose.
                self.pending_recipient_files = Some(files);
                Command::none()
            }

            Message::ChooseRecipient(device) => {
                if let Some(files) = self.pending_recipient_files.take() {
                    self.start_send(device, files)
                } else {
                    Command::none()
                }
            }

            Message::CancelRecipientChooser => {
                self.pending_recipient_files = None;
                Command::none()
            }
            
            Message::VisibilityChanged(visibility) => {
                self.discovery_visibility = visibility;
                let show_all = self
                    .services
                    .config
                    .read()
                    .map(|c| c.show_all_devices)
                    .unwrap_or(false);
                if let Some(selected) = &self.selected_device {
                    if !selected.matches_filter(visibility.device_filter(), show_all) {
                        self.selected_device = None;
                    }
                }
                let services = self.services.clone();
                Command::perform(
                    async move {
                        {
                            let mut cfg = services
                                .config
                                .write()
                                .map_err(|_| "config lock poisoned".to_string())?;
                            cfg.set_discovery_mode(visibility);
                            cfg.save().map_err(|e| e.to_string())?;
                        }
                        services.apply_settings().await.map_err(|e| e.to_string())
                    },
                    |res| match res {
                        Ok(()) => Message::RefreshDevices,
                        Err(e) => Message::Error(e),
                    },
                )
            }

            Message::ShowSettings => {
                self.current_view = AppView::Settings;
                Command::none()
            }

            Message::ShowActivity => {
                self.current_view = AppView::Activity;
                Command::none()
            }

            Message::ShowWebDrop => {
                self.refresh_web_drop(false);
                self.current_view = AppView::WebDrop;
                Command::none()
            }

            Message::RefreshWebDropUrl => {
                let large = matches!(self.current_view, AppView::DjMode);
                self.refresh_web_drop(large);
                Command::none()
            }

            Message::DjScanDrawers => {
                self.scan_dj_drawers();
                Command::none()
            }

            Message::DjDrawerOpen(folder) => {
                let path = self
                    .dj_drawers
                    .iter()
                    .find(|d| d.folder_name == folder)
                    .map(|d| d.path.clone());
                if let Some(path) = path {
                    return Command::perform(
                        async move {
                            open_folder(&path).map_err(|e| e.to_string())
                        },
                        |res| match res {
                            Ok(()) => Message::Tick,
                            Err(e) => Message::Error(e),
                        },
                    );
                }
                Command::none()
            }

            Message::DjDrawerRenameStart(folder) => {
                self.dj_renaming_folder = Some(folder.clone());
                self.dj_rename_text = folder;
                Command::none()
            }

            Message::DjDrawerRenameInput(text) => {
                self.dj_rename_text = text;
                Command::none()
            }

            Message::DjDrawerRenameSubmit => {
                let old = match self.dj_renaming_folder.take() {
                    Some(f) => f,
                    None => return Command::none(),
                };
                let new_label = self.dj_rename_text.trim().to_string();
                self.dj_rename_text.clear();
                if new_label.is_empty() || new_label == old {
                    return Command::none();
                }
                let res = self
                    .services
                    .config
                    .write()
                    .map_err(|_| "config lock poisoned".to_string())
                    .and_then(|mut cfg| {
                        cfg.rename_webdrop_folder(&old, &new_label)
                            .map_err(|e| e.to_string())
                    });
                match res {
                    Ok(_) => {
                        if let Some(i) = self.dj_drawer_order.iter().position(|n| n == &old) {
                            self.dj_drawer_order[i] =
                                crate::config::sanitize_folder_name(&new_label);
                        }
                        self.scan_dj_drawers();
                        Command::none()
                    }
                    Err(e) => {
                        self.dj_renaming_folder = Some(old);
                        Command::perform(async {}, move |_| Message::Error(e))
                    }
                }
            }

            Message::DjDrawerRenameCancel => {
                self.dj_renaming_folder = None;
                self.dj_rename_text.clear();
                Command::none()
            }

            Message::DjDrawerMoveUp(folder) => {
                if let Some(i) = self.dj_drawer_order.iter().position(|n| n == &folder) {
                    if i > 0 {
                        self.dj_drawer_order.swap(i, i - 1);
                        self.apply_dj_drawer_order();
                    }
                }
                Command::none()
            }

            Message::DjDrawerMoveDown(folder) => {
                if let Some(i) = self.dj_drawer_order.iter().position(|n| n == &folder) {
                    if i + 1 < self.dj_drawer_order.len() {
                        self.dj_drawer_order.swap(i, i + 1);
                        self.apply_dj_drawer_order();
                    }
                }
                Command::none()
            }

            Message::ShowDjMode => {
                let prev = self
                    .services
                    .config
                    .read()
                    .map(|c| c.auto_accept_incoming)
                    .unwrap_or(false);
                self.dj_saved_auto_accept = Some(prev);
                self.services.incoming_transfer.set_auto_accept(true);
                self.dj_files_received = 0;
                self.dj_last_file = None;
                self.dj_renaming_folder = None;
                self.dj_rename_text.clear();
                self.scan_dj_drawers();
                self.refresh_web_drop(true);
                self.current_view = AppView::DjMode;
                crate::activity::log(
                    crate::activity::Category::Transfer,
                    "DJ Mode started — QR + set cabinet, auto-save enabled",
                );
                Command::batch([
                    window::maximize(window::Id::MAIN, true),
                    window::gain_focus(window::Id::MAIN),
                ])
            }

            Message::ExitDjMode => {
                if let Some(prev) = self.dj_saved_auto_accept.take() {
                    self.services.incoming_transfer.set_auto_accept(prev);
                }
                self.dj_renaming_folder = None;
                self.dj_rename_text.clear();
                self.current_view = AppView::Main;
                crate::activity::log(
                    crate::activity::Category::Transfer,
                    format!(
                        "DJ Mode ended — {} file(s) received this session",
                        self.dj_files_received
                    ),
                );
                Command::batch([
                    window::maximize(window::Id::MAIN, false),
                    window::gain_focus(window::Id::MAIN),
                ])
            }

            Message::ClearActivityLog => {
                crate::activity::clear();
                Command::none()
            }

            Message::ShowMainView => {
                self.current_view = AppView::Main;
                Command::none()
            }

            Message::BroadcastNameChanged(name) => {
                self.settings_view.set_broadcast_name(name);
                Command::none()
            }

            Message::DownloadDirChanged(path) => {
                self.settings_view.set_download_dir_text(path);
                Command::none()
            }

            Message::BrowseDownloadDir => Command::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .set_title("Choose download folder")
                        .pick_folder()
                        .await
                        .map(|h| h.path().to_path_buf())
                },
                Message::DownloadDirSelected,
            ),

            Message::DownloadDirSelected(path) => {
                if let Some(dir) = path {
                    self.settings_view
                        .set_download_dir_text(dir.display().to_string());
                }
                Command::none()
            }

            Message::MinimizeToTrayChanged(value) => {
                self.settings_view.set_minimize_to_tray(value);
                Command::none()
            }

            Message::AutoAcceptIncomingChanged(value) => {
                self.settings_view.set_auto_accept_incoming(value);
                Command::none()
            }

            Message::ShowAllDevicesChanged(value) => {
                self.settings_view.set_show_all_devices(value);
                if let Ok(mut cfg) = self.services.config.write() {
                    cfg.show_all_devices = value;
                    let _ = cfg.save();
                }
                return Command::perform(
                    Self::fetch_devices(self.services.clone()),
                    Message::DevicesRefreshed,
                );
            }

            Message::SaveSettings => {
                let save_err = {
                    let mut cfg = match self.services.config.write() {
                        Ok(c) => c,
                        Err(_) => return Command::none(),
                    };
                    self.settings_view.apply_to_config(&mut cfg);
                    cfg.save().err()
                };
                if let Some(e) = save_err {
                    self.add_notification(
                        "Settings".to_string(),
                        format!("Could not save settings: {}", e),
                        messages::NotificationType::Error,
                    );
                    return Command::none();
                }

                {
                    let name = self
                        .services
                        .config
                        .read()
                        .map(|c| c.broadcast_name.clone())
                        .unwrap_or_else(|_| crate::config::default_broadcast_name());
                    tray::set_tooltip(&format!("AirDropd — {}", name));
                }

                let services = self.services.clone();
                self.add_notification(
                    "Settings saved".to_string(),
                    "Your preferences have been saved.".to_string(),
                    messages::NotificationType::Success,
                );
                Command::perform(
                    async move {
                        services.apply_settings().await.map_err(|e| e.to_string())
                    },
                    |res| match res {
                        Ok(()) => Message::ShowMainView,
                        Err(e) => Message::Error(e),
                    },
                )
            }

            Message::ResetSettings => {
                let defaults = crate::config::AppConfig::default();
                self.settings_view = views::settings_view::SettingsView::from_config(&defaults);
                Command::none()
            }

            Message::WindowCloseRequested => {
                let minimize = self
                    .services
                    .config
                    .read()
                    .map(|c| c.minimize_to_tray)
                    .unwrap_or(true);
                if minimize {
                    self.window_hidden = true;
                    Command::batch([
                        window::change_mode(window::Id::MAIN, window::Mode::Hidden),
                    ])
                } else {
                    Command::perform(async {}, |_| Message::QuitApp)
                }
            }

            Message::WindowMinimized => {
                let minimize = self
                    .services
                    .config
                    .read()
                    .map(|c| c.minimize_to_tray)
                    .unwrap_or(true);
                if minimize {
                    self.window_hidden = true;
                    Command::batch([
                        window::change_mode(window::Id::MAIN, window::Mode::Hidden),
                    ])
                } else {
                    Command::none()
                }
            }

            Message::ShowWindow => {
                self.window_hidden = false;
                Command::batch([
                    window::change_mode(window::Id::MAIN, window::Mode::Windowed),
                    window::gain_focus(window::Id::MAIN),
                ])
            }

            Message::TrayAction(action) => match action.as_str() {
                "show" => {
                    self.window_hidden = false;
                    Command::batch([
                        window::change_mode(window::Id::MAIN, window::Mode::Windowed),
                        window::gain_focus(window::Id::MAIN),
                    ])
                }
                "quit" => Command::perform(async { std::process::exit(0) }, |_| Message::Tick),
                _ => Command::none(),
            },

            Message::QuitApp => Command::perform(async { std::process::exit(0) }, |_| Message::Tick),

            Message::FileReceived(path) => {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "file".to_string());
                self.add_notification(
                    "File received".to_string(),
                    format!("Saved to AirDropd folder: {}", name),
                    messages::NotificationType::Success,
                );
                Command::none()
            }

            Message::Error(msg) => {
                self.add_notification(
                    "Error".to_string(),
                    msg,
                    messages::NotificationType::Error,
                );
                Command::none()
            }

            Message::PollTray => {
                if let Some(action) = tray::poll_tray_action() {
                    return self.update(Message::TrayAction(action.to_string()));
                }
                Command::none()
            }

            Message::PollIncomingTransfer => {
                // Piggy-back outgoing transfer progress on the same timer.
                if let Ok(slot) = self.services.send_progress.lock() {
                    if let Some(progress) = *slot {
                        self.file_transfer_progress = Some(progress);
                        self.airdrop_status =
                            crate::protocols::airdrop::AirDropStatus::Transferring(progress);
                    }
                }
                let gate = self.services.incoming_transfer.clone();
                Command::perform(async move { gate.peek().await }, Message::UpdatePendingIncoming)
            }

            Message::UpdatePendingIncoming(pending) => {
                let was_pending = self.pending_incoming.is_some();
                self.pending_incoming = pending.clone();
                if pending.is_some() && !was_pending {
                    self.window_hidden = false;
                    return Command::batch([
                        window::change_mode(window::Id::MAIN, window::Mode::Windowed),
                        window::gain_focus(window::Id::MAIN),
                    ]);
                }
                Command::none()
            }

            Message::AcceptIncomingTransfer => {
                let gate = self.services.incoming_transfer.clone();
                self.pending_incoming = None;
                Command::perform(
                    async move {
                        gate.respond(true).await;
                    },
                    |_| Message::Tick,
                )
            }

            Message::RejectIncomingTransfer => {
                let gate = self.services.incoming_transfer.clone();
                self.pending_incoming = None;
                Command::perform(
                    async move {
                        gate.respond(false).await;
                    },
                    |_| Message::Tick,
                )
            }

            Message::PollReceived => {
                let paths: Vec<std::path::PathBuf> = if let Some(rx) = self.received_rx.as_mut() {
                    let mut paths = Vec::new();
                    while let Ok(path) = rx.try_recv() {
                        paths.push(path);
                    }
                    paths
                } else {
                    Vec::new()
                };
                let in_dj = self.current_view == AppView::DjMode;
                for path in paths {
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| path.display().to_string());
                    if in_dj {
                        self.dj_files_received = self.dj_files_received.saturating_add(1);
                        let folder = path
                            .parent()
                            .and_then(|p| p.file_name())
                            .map(|p| p.to_string_lossy().to_string());
                        let display = match folder.as_deref() {
                            Some(f) if f != "AirDropd" && f != "WebDrop" => {
                                format!("{}/{}", f, name)
                            }
                            _ => name.clone(),
                        };
                        self.dj_last_file = Some(display);
                        self.scan_dj_drawers();
                    } else {
                        self.add_notification(
                            "File received".to_string(),
                            format!("Saved: {}", name),
                            messages::NotificationType::Success,
                        );
                    }
                }
                Command::none()
            }

            Message::ShowAbout => {
                self.show_about = true;
                Command::none()
            }

            Message::CloseAbout => {
                self.show_about = false;
                Command::none()
            }

            Message::OpenCashAppDonation => Command::perform(
                async {
                    open_url("https://cash.app/$therealstollie").map_err(|e| e.to_string())
                },
                |res| match res {
                    Ok(()) => Message::Tick,
                    Err(e) => Message::Error(e),
                },
            ),

            Message::OpenLogFolder => {
                let path = crate::config::config_path();
                let folder = path
                    .parent()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(crate::config::config_path);
                Command::perform(
                    async move {
                        open_folder(&folder).map_err(|e| e.to_string())
                    },
                    |res| match res {
                        Ok(()) => Message::Info("Opened AirDropd data folder.".into()),
                        Err(e) => Message::Error(e),
                    },
                )
            }

            Message::OpenReceiveFolder => {
                let folder = self
                    .services
                    .config
                    .read()
                    .map(|c| c.receive_dir())
                    .unwrap_or_else(|_| crate::config::AppConfig::default().receive_dir());
                Command::perform(
                    async move {
                        std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
                        open_folder(&folder).map_err(|e| e.to_string())
                    },
                    |res| match res {
                        Ok(()) => Message::Info("Opened save folder.".into()),
                        Err(e) => Message::Error(e),
                    },
                )
            }

            Message::ClearCache => Command::perform(
                async {
                    let tmp = std::env::temp_dir();
                    let mut removed = 0usize;
                    if let Ok(entries) = std::fs::read_dir(&tmp) {
                        for entry in entries.flatten() {
                            let name = entry.file_name().to_string_lossy().to_string();
                            if name.starts_with("AirDrop") {
                                if std::fs::remove_file(entry.path()).is_ok() {
                                    removed += 1;
                                }
                            }
                        }
                    }
                    Ok::<usize, String>(removed)
                },
                |res| match res {
                    Ok(n) => Message::Info(format!("Removed {n} temporary AirDrop file(s).")),
                    Err(e) => Message::Error(e),
                },
            ),

            Message::RunDiagnostics => {
                let services = self.services.clone();
                Command::perform(
                    async move {
                        let mut lines = Vec::new();
                        lines.push(format!("AirDropd {}", env!("CARGO_PKG_VERSION")));
                        lines.push(format!(
                            "Broadcast: {}",
                            services
                                .config
                                .read()
                                .map(|c| c.broadcast_name.clone())
                                .unwrap_or_else(|_| "unknown".into())
                        ));
                        let ble = services.ble.lock().await;
                        lines.push(format!(
                            "BLE scanning: {}",
                            if ble.is_scanning().await { "yes" } else { "no" }
                        ));
                        lines.push(format!(
                            "BLE advertising: {}",
                            if ble.is_advertising().await { "yes" } else { "no" }
                        ));
                        drop(ble);
                        let awdl = services.awdl.lock().await;
                        let peers = awdl.get_peers().await;
                        lines.push(format!("AWDL peers: {}", peers.len()));
                        lines.join("\n")
                    },
                    Message::Info,
                )
            }

            Message::Info(msg) => {
                self.add_notification(
                    "AirDropd".to_string(),
                    msg,
                    messages::NotificationType::Info,
                );
                Command::none()
            }

            Message::OpenWebsite => Command::perform(
                async {
                    open_url("https://www.rhythmicrecords.net").map_err(|e| e.to_string())
                },
                |res| match res {
                    Ok(()) => Message::Tick,
                    Err(e) => Message::Error(e),
                },
            ),

            Message::OpenDocumentation => Command::perform(
                async {
                    open_url("https://github.com/gigguru/AirDropd#readme")
                        .map_err(|e| e.to_string())
                },
                |res| match res {
                    Ok(()) => Message::Tick,
                    Err(e) => Message::Error(e),
                },
            ),

            Message::OpenIssues => Command::perform(
                async {
                    open_url("https://github.com/gigguru/AirDropd/issues")
                        .map_err(|e| e.to_string())
                },
                |res| match res {
                    Ok(()) => Message::Tick,
                    Err(e) => Message::Error(e),
                },
            ),

            // Handle all other message variants with a wildcard pattern
            _ => Command::none(),
        }
    }

    fn view(&self) -> Element<Self::Message> {
        if self.show_about {
            return views::about_view::overlay(&self.theme);
        }
        match self.current_view {
            AppView::Splash => views::splash_view::render(&self.splash_frames, self.splash_tick),
            AppView::Loading => self.loading_view(),
            AppView::Main => self.main_view(),
            AppView::Activity => self.activity_view(),
            AppView::WebDrop => self.web_drop_view(),
            AppView::DjMode => self.dj_mode_view(),
            AppView::Settings => self.settings_view(),
        }
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        let refresh_secs = if self.is_scanning { 2 } else { 4 };
        // No discovery refreshes or radar redraws while minimized to tray.
        let refresh = if matches!(self.current_view, AppView::Main)
            && !self.window_hidden
            && !self.discovery_frozen
        {
            time::every(Duration::from_secs(refresh_secs)).map(|_| Message::RefreshDevices)
        } else {
            Subscription::none()
        };

        let sonar = if matches!(self.current_view, AppView::Main)
            && self.device_view_mode == views::device_list_view::DeviceViewMode::Sonar
            && !self.discovery_frozen
            && !self.window_hidden
        {
            time::every(Duration::from_millis(120)).map(|_| Message::SonarTick)
        } else {
            Subscription::none()
        };

        let tray_poll = time::every(Duration::from_millis(300)).map(|_| Message::PollTray);

        let received_poll =
            time::every(Duration::from_millis(500)).map(|_| Message::PollReceived);

        let incoming_poll =
            time::every(Duration::from_millis(250)).map(|_| Message::PollIncomingTransfer);

        let webdrop_refresh = if matches!(self.current_view, AppView::WebDrop | AppView::DjMode) {
            time::every(Duration::from_secs(3)).map(|_| Message::RefreshWebDropUrl)
        } else {
            Subscription::none()
        };

        let dj_drawers = if matches!(self.current_view, AppView::DjMode) && !self.window_hidden {
            time::every(Duration::from_secs(2)).map(|_| Message::DjScanDrawers)
        } else {
            Subscription::none()
        };

        let window_events = event::listen_with(|event, _status| match event {
            Event::Window(_id, window::Event::CloseRequested) => {
                Some(Message::WindowCloseRequested)
            }
            Event::Window(_id, window::Event::FileHovered(_)) => {
                Some(Message::FilesHoveringWindow(true))
            }
            Event::Window(_id, window::Event::FilesHoveredLeft) => {
                Some(Message::FilesHoveringWindow(false))
            }
            Event::Window(_id, window::Event::FileDropped(path)) => {
                Some(Message::FileDroppedOnWindow(path))
            }
            _ => None,
        });

        let splash = if matches!(self.current_view, AppView::Splash) {
            time::every(Duration::from_millis(assets::SPLASH_TICK_MS))
                .map(|_| Message::SplashTick)
        } else {
            Subscription::none()
        };

        Subscription::batch([
            refresh,
            sonar,
            tray_poll,
            received_poll,
            incoming_poll,
            webdrop_refresh,
            dj_drawers,
            window_events,
            splash,
        ])
    }

    fn theme(&self) -> Self::Theme {
        match self.theme {
            Theme::Light => IcedTheme::Light,
            Theme::Dark => IcedTheme::Dark,
        }
    }
}

impl AirDropdApp {
    /// Loading view.
    fn loading_view(&self) -> Element<Message> {
        components::loading_state(&self.status_message)
    }

    /// Main application view.
    fn main_view(&self) -> Element<Message> {
        views::main_view::render(
            &self.discovered_devices,
            self.selected_device.as_ref(),
            self.device_view_mode,
            self.list_sort_column,
            self.list_sort_ascending,
            self.is_scanning,
            self.sonar_tick,
            &self.airdrop_status,
            self.file_transfer_progress,
            &self.notifications,
            self.show_link_dialog,
            &self.link_url,
            self.pending_incoming.as_ref(),
            self.pending_recipient_files.as_deref(),
            self.drop_hover,
            self.discovery_visibility,
            self.discovery_frozen,
            &self.theme,
        )
    }
 
    /// Settings view.
    fn settings_view(&self) -> Element<Message> {
        self.settings_view.view(&self.theme)
    }

    /// Recompute the Web Drop URL + QR for the current network address.
    fn refresh_web_drop(&mut self, large: bool) {
        self.web_drop_listening = self.services.web_drop.is_listening();
        match self.services.web_drop.qr_url() {
            Ok(url) => {
                let (module_px, quiet) = if large { (14, 4) } else { (8, 4) };
                let png = qr::png_bytes(&url, module_px, quiet).ok();
                if large {
                    self.dj_qr = png.map(iced::widget::image::Handle::from_memory);
                } else {
                    self.web_drop_qr = png.map(iced::widget::image::Handle::from_memory);
                }
                self.web_drop_url = Some(url);
            }
            Err(_) => {
                self.web_drop_url = None;
                self.web_drop_qr = None;
                self.dj_qr = None;
            }
        }
    }

    /// QR-based Web Drop receive screen.
    fn web_drop_view(&self) -> Element<Message> {
        let status = views::webdrop_view::WebDropStatus {
            url: self.web_drop_url.clone(),
            qr: self.web_drop_qr.clone(),
            server_listening: self.web_drop_listening,
        };
        views::webdrop_view::render(&status, &self.theme)
    }

    /// Full-screen DJ receive mode.
    fn dj_mode_view(&self) -> Element<Message> {
        let device_name = self
            .services
            .config
            .read()
            .map(|c| c.broadcast_name.clone())
            .unwrap_or_else(|_| "this PC".to_string());
        let status = views::dj_mode_view::DjModeStatus {
            device_name,
            url: self.web_drop_url.clone(),
            qr: self.dj_qr.clone(),
            server_listening: self.web_drop_listening,
            files_received: self.dj_files_received,
            last_file: self.dj_last_file.clone(),
            drawers: &self.dj_drawers,
            renaming_folder: self.dj_renaming_folder.as_deref(),
            rename_text: &self.dj_rename_text,
        };
        views::dj_mode_view::render(&status, &self.theme)
    }

    /// Live Activity protocol-event feed.
    fn activity_view(&self) -> Element<Message> {
        let (broadcast_name, discoverable) = self
            .services
            .config
            .read()
            .map(|c| (c.broadcast_name.clone(), c.discoverable))
            .unwrap_or_else(|_| ("AirDropd".to_string(), true));
        let status = views::activity_view::ReceiverStatus {
            broadcast_name,
            address: crate::network::util::primary_ipv4()
                .ok()
                .map(|ip| ip.to_string()),
            discoverable,
        };
        views::activity_view::render(status, &self.theme)
    }
  
    /// Fetch discovered Apple devices from mDNS and BLE.
    ///
    /// /Discover probe results are cached per endpoint: each probe opens a
    /// TLS connection, so re-probing every refresh tick would hammer nearby
    /// devices and slow the UI loop down.
    async fn fetch_devices(
        services: Arc<crate::AirDropdServices>,
    ) -> Vec<crate::network::DiscoveredDevice> {
        let (our_name, our_ip, show_all, discovery_mode) = {
            let cfg = services.config.read().ok();
            let name = cfg
                .as_ref()
                .map(|c| c.broadcast_name.to_lowercase())
                .unwrap_or_default();
            let show_all = cfg.as_ref().map(|c| c.show_all_devices).unwrap_or(false);
            let discovery_mode = cfg
                .as_ref()
                .map(|c| c.discovery_mode)
                .unwrap_or(crate::config::DiscoveryMode::Everyone);
            let ip = crate::network::util::primary_ipv4().ok();
            (name, ip, show_all, discovery_mode)
        };
        let include_accessories = discovery_mode.include_accessories(show_all);
        let device_filter = discovery_mode.device_filter();

        let mut by_key: HashMap<String, crate::network::DiscoveredDevice> = HashMap::new();
        let mut by_name: HashMap<String, String> = HashMap::new();

        if let Ok(devices) = services.device_discovery.lock().await.get_devices().await {
            let mut collected: Vec<crate::network::DiscoveredDevice> = devices
                .into_iter()
                .filter(|device| {
                    !device.name.is_empty()
                        && device.name.to_lowercase() != our_name
                        && our_ip
                            .map(|ip| device.address != std::net::IpAddr::V4(ip))
                            .unwrap_or(true)
                })
                .collect();

            let cache = probe_name_cache();
            let mut resolved: Vec<(usize, String)> = Vec::new();
            let mut probe_futures = Vec::new();
            for (idx, device) in collected.iter().enumerate() {
                if matches!(
                    device.service_type,
                    crate::network::ServiceType::AirDrop | crate::network::ServiceType::Companion
                ) && device.port > 0
                    && !device.address.is_unspecified()
                {
                    let addr = std::net::SocketAddr::new(device.address, device.port);
                    let cache_key = addr.to_string();

                    let cached = cache.lock().ok().and_then(|c| {
                        c.get(&cache_key).and_then(|(probed_at, name)| {
                            (probed_at.elapsed() < PROBE_CACHE_TTL).then(|| name.clone())
                        })
                    });
                    if let Some(name) = cached {
                        if let Some(name) = name {
                            resolved.push((idx, name));
                        }
                        continue;
                    }

                    probe_futures.push(async move {
                        let name = tokio::time::timeout(
                            Duration::from_millis(1500),
                            crate::protocols::airdrop_client::AirDropClient::probe_discover(addr),
                        )
                        .await
                        .ok()
                        .and_then(|r| r.ok())
                        .flatten();
                        (idx, cache_key, name)
                    });
                }
            }

            for (idx, cache_key, name) in futures::future::join_all(probe_futures).await {
                if let Ok(mut c) = cache.lock() {
                    c.insert(cache_key, (std::time::Instant::now(), name.clone()));
                }
                if let Some(name) = name {
                    resolved.push((idx, name));
                }
            }

            for (idx, name) in resolved {
                collected[idx].name = name;
            }

            for device in collected {
                let key = format!("{}:{}", device.address, device.port);
                if device.port > 0 {
                    by_name.insert(device.name.to_lowercase(), key.clone());
                }
                by_key.insert(key, device);
            }
        }

        let ble_devices = services.ble.lock().await.get_discovered_devices().await;
        for ble in ble_devices {
            if ble.name.to_lowercase() == our_name && !ble.name.is_empty() {
                continue;
            }
            let rssi = if ble.rssi != 0 { Some(ble.rssi) } else { None };

            if !ble.name.is_empty() {
                // Device also found over Wi-Fi: attach the BLE signal strength
                // so the radar can place it by physical distance.
                if let Some(key) = by_name.get(&ble.name.to_lowercase()) {
                    if let Some(device) = by_key.get_mut(key) {
                        device.rssi = rssi;
                    }
                    continue;
                }
            }

            // Accessories (AirPods, AirTags, Find My beacons) only show when
            // "Show all nearby devices" is enabled — the lost-device finder.
            if ble.accessory_label.is_some() && !include_accessories {
                continue;
            }

            // iPhones and iPads never include a name in their Continuity
            // beacons and only advertise AirDrop over AWDL (not regular
            // Wi-Fi), so a nameless Apple beacon is how an iPhone looks
            // from Windows. Surface it instead of dropping it.
            let name = if !ble.name.is_empty() {
                ble.name.clone()
            } else if let Some(label) = ble.accessory_label {
                format!("{} {}", label, short_ble_suffix(&ble.id))
            } else if ble.mobile_profile.is_mobile {
                anonymous_mobile_ble_name(&ble.mobile_profile, &ble.id)
            } else if ble.apple {
                format!("Apple device {}", short_ble_suffix(&ble.id))
            } else {
                continue;
            };

            let mut txt_records = HashMap::new();
            txt_records.insert("ble_id".to_string(), ble.id.clone());
            if ble.name.is_empty() {
                txt_records.insert("anonymous".to_string(), "1".to_string());
            }
            if ble.airdrop_active {
                txt_records.insert("airdrop_active".to_string(), "1".to_string());
            }
            if ble.accessory_label.is_some() {
                txt_records.insert("accessory".to_string(), "1".to_string());
            }
            if let Some(label) = ble.accessory_label {
                txt_records.insert("accessory_label".to_string(), label.to_string());
            }
            if ble.mobile_profile.is_mobile {
                txt_records.insert("mobile_presence".to_string(), "1".to_string());
                txt_records.insert(
                    "device_class".to_string(),
                    ble.mobile_profile.device_class.to_string(),
                );
                match ble.mobile_profile.platform {
                    Some(crate::network::discovery::BleMobilePlatform::Android) => {
                        txt_records.insert("platform".to_string(), "android".to_string());
                    }
                    Some(crate::network::discovery::BleMobilePlatform::Apple) => {
                        txt_records.insert("apple_presence".to_string(), "1".to_string());
                    }
                    None => {}
                }
            } else if !ble.apple {
                txt_records.insert("platform".to_string(), "android".to_string());
                if let Some(class) = crate::network::discovery::android_class_from_ble(
                    &name,
                    ble.manufacturer_data.keys().copied(),
                ) {
                    txt_records.insert("device_class".to_string(), class.to_string());
                }
            } else if ble.apple && ble.name.is_empty() && ble.accessory_label.is_none() {
                txt_records.insert("apple_presence".to_string(), "1".to_string());
            }

            let key = format!("ble:{}", ble.id);
            by_key.entry(key).or_insert(crate::network::DiscoveredDevice {
                name,
                address: std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED),
                port: 0,
                service_type: crate::network::ServiceType::AirDrop,
                txt_records,
                rssi,
            });
        }

        let mut devices: Vec<_> = by_key.into_values().collect();

        // One row per device — mDNS often reports AirPlay, RAOP, companion, etc. separately.
        let mut best_by_name: HashMap<String, crate::network::DiscoveredDevice> = HashMap::new();
        let dedup_key = |device: &crate::network::DiscoveredDevice| {
            let base = device.name.to_lowercase();
            if device.txt_records.get("anonymous") == Some(&"1".to_string()) {
                if let Some(id) = device.txt_records.get("ble_id") {
                    return format!("{base}|{id}");
                }
            }
            base
        };
        for device in devices {
            let key = dedup_key(&device);
            let rank = |d: &crate::network::DiscoveredDevice| {
                let service = match d.service_type {
                    crate::network::ServiceType::AirDrop => 0,
                    crate::network::ServiceType::Companion => 1,
                    crate::network::ServiceType::DeviceInfo => 2,
                    crate::network::ServiceType::AirPlay => 3,
                    crate::network::ServiceType::Raop => 4,
                    _ => 5,
                };
                let reachable = if !d.address.is_unspecified() && d.port > 0 {
                    0
                } else {
                    16
                };
                reachable + service
            };
            best_by_name
                .entry(key)
                .and_modify(|existing| {
                    let rssi = match (device.rssi, existing.rssi) {
                        (Some(a), Some(b)) => Some(a.max(b)),
                        (a, b) => a.or(b),
                    };
                    // Merge TXT records from every advertisement: the hardware
                    // model (used for the device-type icon) often comes from a
                    // different service than the one we keep for transfers.
                    let mut txt = existing.txt_records.clone();
                    for (k, v) in &device.txt_records {
                        txt.entry(k.clone()).or_insert_with(|| v.clone());
                    }
                    if rank(&device) < rank(existing) {
                        *existing = device.clone();
                    }
                    existing.rssi = rssi;
                    existing.txt_records = txt;
                })
                .or_insert(device);
        }
        devices = best_by_name.into_values().collect();

        // Drop speakers/TVs that only advertise AirPlay audio, but keep phones
        // that surface through AirPlay / RAOP with no companion-link record.
        devices.retain(|d| {
            if matches!(
                d.service_type,
                crate::network::ServiceType::AirPlay | crate::network::ServiceType::Raop
            ) {
                return d.is_mobile_device();
            }
            true
        });

        devices.sort_by(|a, b| {
            let rank = |d: &crate::network::DiscoveredDevice| match d.service_type {
                crate::network::ServiceType::AirDrop => 0,
                crate::network::ServiceType::Companion => 1,
                crate::network::ServiceType::DeviceInfo => 2,
                _ => 3,
            };
            rank(a)
                .cmp(&rank(b))
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });

        devices.retain(|d| d.matches_filter(device_filter, show_all));
        devices
    }

    /// Kick off a real AirDrop transfer of files/folders to a device.
    fn start_send(
        &mut self,
        device: crate::network::DiscoveredDevice,
        paths: Vec<std::path::PathBuf>,
    ) -> Command<Message> {
        if paths.is_empty() {
            return Command::none();
        }
        if device.address.is_unspecified() || device.port == 0 {
            self.add_notification(
                "Cannot send yet".to_string(),
                format!(
                    "{} is visible via Bluetooth only — join the same Wi‑Fi network to transfer.",
                    device.name
                ),
                messages::NotificationType::Error,
            );
            return Command::none();
        }

        let count = paths.len();
        let label = if count == 1 {
            paths[0]
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "1 item".to_string())
        } else {
            format!("{} items", count)
        };
        self.add_notification(
            "Sending".to_string(),
            format!("{} → {}", label, device.name),
            messages::NotificationType::Info,
        );
        self.airdrop_status = crate::protocols::airdrop::AirDropStatus::Connecting;
        self.file_transfer_progress = Some(0.0);

        let service_id = self
            .services
            .config
            .read()
            .map(|c| c.service_id.clone())
            .unwrap_or_default();
        let progress = self.services.send_progress.clone();
        let addr = std::net::SocketAddr::new(device.address, device.port);

        Command::perform(
            async move {
                crate::protocols::airdrop_client::AirDropClient::send_files(
                    addr,
                    paths,
                    &service_id,
                    progress,
                )
                .await
                .map_err(|e| e.to_string())
            },
            Message::FileSendCompleted,
        )
    }

    /// Add a notification to the list.
    fn add_notification(
        &mut self,
        title: String,
        message: String,
        notification_type: messages::NotificationType,
    ) {
        let notification = messages::NotificationMessage {
            title,
            content: message,
            notification_type,
            duration_ms: Some(3000),
        };
        
        self.notifications.push(notification);
        
        // Keep only the latest 5 notifications.
        if self.notifications.len() > 5 {
            self.notifications.remove(0);
        }
    }

    /// Keep the open device form synced with the latest discovery snapshot.
    fn sync_selected_device(&mut self) {
        let Some(selected) = self.selected_device.as_ref() else {
            return;
        };
        let key = selected.match_key();
        self.selected_device = self
            .discovered_devices
            .iter()
            .find(|d| d.match_key() == key)
            .cloned();
    }

    /// Rescan WebDrop guest folders for the DJ set cabinet.
    fn scan_dj_drawers(&mut self) {
        let root = match self.services.config.read() {
            Ok(cfg) => cfg.webdrop_dir(),
            Err(_) => return,
        };
        let mut found: Vec<views::dj_mode_view::DjDrawer> = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let folder_name = entry.file_name().to_string_lossy().to_string();
                let file_count = count_files_in_dir(&path);
                found.push(views::dj_mode_view::DjDrawer {
                    folder_name,
                    path,
                    file_count,
                });
            }
        }
        found.sort_by(|a, b| a.folder_name.to_lowercase().cmp(&b.folder_name.to_lowercase()));

        if self.dj_drawer_order.is_empty() {
            self.dj_drawer_order = found.iter().map(|d| d.folder_name.clone()).collect();
        } else {
            for drawer in &found {
                if !self.dj_drawer_order.contains(&drawer.folder_name) {
                    self.dj_drawer_order.push(drawer.folder_name.clone());
                }
            }
            self.dj_drawer_order
                .retain(|name| found.iter().any(|d| d.folder_name == *name));
        }

        let mut ordered = Vec::with_capacity(found.len());
        for name in &self.dj_drawer_order {
            if let Some(drawer) = found.iter().find(|d| d.folder_name == *name) {
                ordered.push(drawer.clone());
            }
        }
        self.dj_drawers = ordered;
    }

    fn apply_dj_drawer_order(&mut self) {
        let map: std::collections::HashMap<_, _> = self
            .dj_drawers
            .drain(..)
            .map(|d| (d.folder_name.clone(), d))
            .collect();
        self.dj_drawers = self
            .dj_drawer_order
            .iter()
            .filter_map(|name| map.get(name).cloned())
            .collect();
    }
}

fn count_files_in_dir(path: &std::path::Path) -> usize {
    std::fs::read_dir(path)
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| e.path().is_file())
                .count()
        })
        .unwrap_or(0)
}

fn open_folder(path: &std::path::Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        std::process::Command::new("explorer")
            .arg(path)
            .spawn()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    #[cfg(not(windows))]
    {
        std::process::Command::new("open")
            .arg(path)
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

fn open_url(url: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    #[cfg(not(windows))]
    {
        std::process::Command::new("open")
            .arg(url)
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

/// Prefer platform-native GPU backends when the user has not set `WGPU_BACKEND`.
fn configure_render_backend() {
    std::env::set_var("WGPU_VALIDATION", "0");
    if std::env::var_os("WGPU_BACKEND").is_some() {
        return;
    }
    #[cfg(windows)]
    std::env::set_var("WGPU_BACKEND", "dx12");
}

fn app_window_settings(icon: Option<iced::window::Icon>) -> iced::window::Settings {
    iced::window::Settings {
        size: iced::Size::new(680.0, 560.0),
        min_size: Some(iced::Size::new(560.0, 480.0)),
        position: iced::window::Position::Centered,
        resizable: true,
        decorations: true,
        transparent: false,
        icon,
        ..Default::default()
    }
}

/// Main function for starting the application.
pub fn run(services: Arc<crate::AirDropdServices>) -> iced::Result {
    configure_render_backend();

    let window_icon = assets::load_window_icon();

    AirDropdApp::run(Settings {
        flags: services,
        id: None,
        fonts: Vec::new(),
        window: app_window_settings(window_icon),
        default_font: iced::Font::DEFAULT,
        default_text_size: iced::Pixels(13.0),
        antialiasing: true,
    })
}

/// Start the AirDropd application with the provided services.
pub async fn run_app(
    _services: std::sync::Arc<crate::AirDropdServices>,
) -> Result<(), Box<dyn std::error::Error>> {
    configure_render_backend();

    let window_icon = assets::load_window_icon();

    AirDropdApp::run(Settings {
        flags: _services,
        id: None,
        fonts: Vec::new(),
        window: app_window_settings(window_icon),
        default_font: iced::Font::DEFAULT,
        default_text_size: iced::Pixels(13.0),
        antialiasing: true,
    })?;
    Ok(())
}

/// Utility macro for creating elements with spacing.
#[macro_export]
macro_rules! spaced {
    ($spacing:expr, $($element:expr),+ $(,)?) => {
        iced::widget::column![$($element),+].spacing($spacing)
    };
}

/// Utility macro for creating rows with spacing.
#[macro_export]
macro_rules! spaced_row {
    ($spacing:expr, $($element:expr),+ $(,)?) => {
        iced::widget::row![$($element),+].spacing($spacing)
    };
}
