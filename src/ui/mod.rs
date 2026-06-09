//! Modulo principale dell'interfaccia utente Iced
//!
//! Questo modulo contiene l'implementazione completa dell'interfaccia utente
//! utilizzando la libreria Iced, con design moderno e reattivo.

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

// Moduli pub mod app;
pub mod components;
pub mod assets;
pub mod messages;
pub mod styles;
pub mod tray;
pub mod views;
pub mod widgets;

// Re-export dei tipi principali
pub use messages::Message;
 
/// Tema dell'applicazione (utilizzato da `styles` per gli stili personalizzati)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Theme {
    #[default]
    Dark,
    Light,
}
 
/// Struttura principale dell'applicazione AirDropd
pub struct AirDropdApp {
    /// Background services (mDNS, BLE, AirDrop server)
    services: Arc<crate::AirDropdServices>,

    /// Stato corrente dell'applicazione
    current_view: AppView,
    
    /// Dispositivi scoperti nella rete
    discovered_devices: Vec<crate::network::DiscoveredDevice>,
    
    /// Dispositivo attualmente selezionato
    selected_device: Option<crate::network::DiscoveredDevice>,
    
    /// Stato della scansione
    is_scanning: bool,
    
    /// Stato AirPlay
    airplay_status: crate::protocols::airplay::AirPlayStatus,
    
    /// Stato AirDrop
    airdrop_status: crate::protocols::airdrop::AirDropStatus,
    
    /// Progresso del trasferimento file (0.0-100.0)
    file_transfer_progress: Option<f32>,
    
    /// Notificazioni attive
    notifications: Vec<messages::NotificationMessage>,
    
    /// Tema corrente
    theme: Theme,
    
    /// Discovery visibility (macOS AirDrop-style)
    discovery_visibility: views::settings_view::AirDropVisibility,
    
    /// Vista impostazioni persistita per evitare problemi di lifetime
    settings_view: views::settings_view::SettingsView,
    
    /// Vista informazioni persistita per evitare problemi di lifetime
    about_view: views::about_view::AboutView,
    
    /// Stato del dialog per l'invio di link
    show_link_dialog: bool,
    
    /// URL da inviare tramite link
    link_url: String,
    
    /// Stato di caricamento generale
    is_loading: bool,
    
    /// Messaggio di stato
    status_message: String,

    /// Main window hidden in system tray
    window_hidden: bool,

    /// Subscription to incoming file notifications
    received_rx: Option<tokio::sync::broadcast::Receiver<std::path::PathBuf>>,

    /// Splash animation state
    splash_frames: assets::SplashFrames,
    splash_tick: usize,
}

/// Viste disponibili nell'applicazione
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppView {
    /// Vista principale con lista dispositivi e pannello azioni
    Main,
    /// Vista delle impostazioni
    Settings,
    /// Vista informazioni sull'app
    About,
    /// Vista di caricamento iniziale
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
        let tray_name = cfg.broadcast_name.clone();
        let settings_view = views::settings_view::SettingsView::from_config(&cfg);
        let received_rx = services.received_tx.subscribe();

        let _ = tray::init_tray(&format!("AirDropd — {}", tray_name));

        let app = Self {
            services,
            current_view: AppView::Splash,
            status_message: "Starting...".to_string(),
            is_loading: true,
            theme: Theme::default(),
            discovery_visibility: views::settings_view::AirDropVisibility::Everyone,
            settings_view,
            about_view: views::about_view::AboutView::new(
                "0.1.0".to_string(),
                "unknown".to_string(),
                None,
            ),
            discovered_devices: Vec::new(),
            selected_device: None,
            is_scanning: false,
            airplay_status: crate::protocols::airplay::AirPlayStatus::Idle,
            airdrop_status: crate::protocols::airdrop::AirDropStatus::Idle,
            file_transfer_progress: None,
            notifications: Vec::new(),
            show_link_dialog: false,
            link_url: String::new(),
            window_hidden: false,
            received_rx: Some(received_rx),
            splash_frames: assets::SplashFrames::new(),
            splash_tick: 0,
        };

        (app, Command::none())
    }

    fn title(&self) -> String {
        match self.current_view {
            AppView::Main => "AirDropd".to_string(),
            AppView::Settings => "AirDropd — Settings".to_string(),
            AppView::About => "AirDropd — About".to_string(),
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
                Command::perform(async {}, |_| Message::StartScanning)
            }

            Message::InitializationComplete => {
                self.current_view = AppView::Main;
                self.is_loading = false;
                self.status_message = "Ready".to_string();
                
                // Avvia la scansione automatica
                Command::perform(
                    async { () },
                    |_| Message::StartScanning,
                )
            }

            Message::StartScanning => {
                self.is_scanning = true;
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
                self.discovered_devices = devices;
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
                self.discovered_devices = devices;
                self.is_scanning = false;
                self.status_message = format!(
                    "Found {} devices",
                    self.discovered_devices.len()
                );
                Command::none()
            }

            Message::DeviceSelected(device) => {
                self.selected_device = Some(device.clone());
                self.status_message = format!("Selected: {}", device.name);
                
                self.add_notification(
                    "Device selected".to_string(),
                    format!("You can now send content to {}", device.name),
                    messages::NotificationType::Info,
                );
                
                Command::none()
            }

            Message::SendFile(device) => {
                let device = device.clone();
                Command::perform(
                    async move {
                        use rfd::AsyncFileDialog;
                        let file = AsyncFileDialog::new().pick_file().await;
                        let Some(handle) = file else {
                            return Err("No file selected".to_string());
                        };
                        let path = handle.path().to_path_buf();
                        let port = match device.service_type {
                            crate::network::ServiceType::AirDrop
                            | crate::network::ServiceType::Companion => {
                                if device.port > 0 { device.port } else { 8770 }
                            }
                            _ => if device.port > 0 { device.port } else { 8770 },
                        };
                        if device.address.is_unspecified() {
                            return Err(format!(
                                "{} is visible via Bluetooth only — wait for Wi‑Fi discovery or move closer on the same network.",
                                device.name
                            ));
                        }
                        let addr = std::net::SocketAddr::new(device.address, port);
                        crate::protocols::airdrop_client::AirDropClient::send_file(addr, &path)
                            .await
                            .map_err(|e| e.to_string())?;
                        Ok::<(), String>(())
                    },
                    |res| match res {
                        Ok(()) => Message::FileSendCompleted(Ok(())),
                        Err(e) => Message::FileSendCompleted(Err(e)),
                    },
                )
            }

            Message::SendLink(device, url) => {
                self.link_url = url.clone();
                self.add_notification(
                    "Sending link".to_string(),
                    format!("Sending link to {}", device.name),
                    messages::NotificationType::Info,
                );
                self.airdrop_status = crate::protocols::airdrop::AirDropStatus::Connecting;
                Command::perform(
                    async {
                        tokio::time::sleep(Duration::from_secs(2)).await;
                        Ok::<(), String>(())
                    },
                    |res| match res {
                        Ok(()) => Message::FileSendCompleted(Ok(())),
                        Err(e) => Message::FileSendCompleted(Err(e)),
                    },
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
                    Ok(()) => self.add_notification(
                        "Transfer complete".to_string(),
                        "Operation completed successfully".to_string(),
                        messages::NotificationType::Success,
                    ),
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
                
                // Auto-rimuovi notifica dopo 5 secondi
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
            Message::StartScreenMirroring(_device) => {
                self.airplay_status = crate::protocols::airplay::AirPlayStatus::Connecting;
                Command::perform(
                    async {
                        tokio::time::sleep(Duration::from_secs(3)).await;
                        crate::protocols::airplay::AirPlayStatus::Connected
                    },
                    Message::AirPlayStatusChanged,
                )
            }

            Message::StopScreenMirroring => {
                self.airplay_status = crate::protocols::airplay::AirPlayStatus::Idle;
                Command::none()
            }

            Message::AirPlayStatusChanged(status) => {
                self.airplay_status = status.clone();
                match status {
                    crate::protocols::airplay::AirPlayStatus::Connected => self.add_notification(
                        "AirPlay connected".to_string(),
                        "AirPlay connection established".to_string(),
                        messages::NotificationType::Success,
                    ),
                    crate::protocols::airplay::AirPlayStatus::Failed(err) => self.add_notification(
                        "AirPlay error".to_string(),
                        err,
                        messages::NotificationType::Error,
                    ),
                    _ => {}
                }
                Command::none()
            }
            
            Message::VisibilityChanged(visibility) => {
                self.discovery_visibility = visibility;
                let services = self.services.clone();
                Command::perform(
                    async move {
                        let discoverable =
                            visibility != views::settings_view::AirDropVisibility::ReceivingOff;
                        {
                            let mut cfg = services
                                .config
                                .write()
                                .map_err(|_| "config lock poisoned".to_string())?;
                            cfg.discoverable = discoverable;
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
                for path in paths {
                    self.add_notification(
                        "File received".to_string(),
                        format!(
                            "Saved: {}",
                            path.file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_else(|| path.display().to_string())
                        ),
                        messages::NotificationType::Success,
                    );
                }
                Command::none()
            }

            Message::ShowAbout => {
                self.current_view = AppView::About;
                Command::none()
            }

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
                    open_url("https://github.com/gigguru/AirDropd").map_err(|e| e.to_string())
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
        match self.current_view {
            AppView::Splash => views::splash_view::render(&self.splash_frames, self.splash_tick),
            AppView::Loading => self.loading_view(),
            AppView::Main => self.main_view(),
            AppView::Settings => self.settings_view(),
            AppView::About => self.about_view(),
        }
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        let refresh = if matches!(self.current_view, AppView::Main) {
            time::every(Duration::from_secs(3)).map(|_| Message::RefreshDevices)
        } else {
            Subscription::none()
        };

        let tray_poll = time::every(Duration::from_millis(300)).map(|_| Message::PollTray);

        let received_poll =
            time::every(Duration::from_millis(500)).map(|_| Message::PollReceived);

        let window_events = event::listen_with(|event, _status| {
            if let Event::Window(_id, window::Event::CloseRequested) = event {
                Some(Message::WindowCloseRequested)
            } else {
                None
            }
        });

        let splash = if matches!(self.current_view, AppView::Splash) {
            time::every(Duration::from_millis(assets::SPLASH_TICK_MS))
                .map(|_| Message::SplashTick)
        } else {
            Subscription::none()
        };

        Subscription::batch([refresh, tray_poll, received_poll, window_events, splash])
    }

    fn theme(&self) -> Self::Theme {
        match self.theme {
            Theme::Light => IcedTheme::Light,
            Theme::Dark => IcedTheme::Dark,
        }
    }
}

impl AirDropdApp {
    /// Vista di caricamento
    fn loading_view(&self) -> Element<Message> {
        components::loading_state(&self.status_message)
    }

    /// Vista principale dell'applicazione
    fn main_view(&self) -> Element<Message> {
        views::main_view::render(
            &self.discovered_devices,
            self.selected_device.as_ref(),
            self.is_scanning,
            &self.airplay_status,
            &self.airdrop_status,
            self.file_transfer_progress,
            &self.notifications,
            self.show_link_dialog,
            &self.link_url,
            self.discovery_visibility,
            &self.theme,
        )
    }
 
    /// Vista impostazioni
    fn settings_view(&self) -> Element<Message> {
        self.settings_view.view(&self.theme)
    }

    /// Vista informazioni
    fn about_view(&self) -> Element<Message> {
        self.about_view.view(&self.theme)
    }
  
    /// Fetch discovered Apple devices from mDNS and BLE.
    async fn fetch_devices(
        services: Arc<crate::AirDropdServices>,
    ) -> Vec<crate::network::DiscoveredDevice> {
        let (our_name, our_ip) = {
            let cfg = services.config.read().ok();
            let name = cfg
                .as_ref()
                .map(|c| c.broadcast_name.to_lowercase())
                .unwrap_or_default();
            let ip = crate::network::util::primary_ipv4().ok();
            (name, ip)
        };

        let mut by_key: HashMap<String, crate::network::DiscoveredDevice> = HashMap::new();
        let mut by_name: HashMap<String, String> = HashMap::new();

        if let Ok(devices) = services.device_discovery.lock().await.get_devices().await {
            for device in devices {
                if device.name.is_empty() {
                    continue;
                }
                if device.name.to_lowercase() == our_name {
                    continue;
                }
                if let Some(ip) = our_ip {
                    if device.address == std::net::IpAddr::V4(ip) {
                        continue;
                    }
                }
                let key = format!("{}:{}", device.address, device.port);
                if device.port > 0 {
                    by_name.insert(device.name.to_lowercase(), key.clone());
                }
                by_key.insert(key, device);
            }
        }

        let ble_devices = services.ble.lock().await.get_discovered_devices().await;
        for ble in ble_devices {
            if ble.name.is_empty() || ble.name.to_lowercase() == our_name {
                continue;
            }
            if by_name.contains_key(&ble.name.to_lowercase()) {
                continue;
            }
            let key = format!("ble:{}", ble.id);
            by_key.entry(key).or_insert(crate::network::DiscoveredDevice {
                name: ble.name,
                address: std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED),
                port: 0,
                service_type: crate::network::ServiceType::AirDrop,
                txt_records: HashMap::new(),
            });
        }

        let awdl = services.awdl.lock().await;
        for peer in awdl.get_peers().await {
            if peer.device_name.to_lowercase() == our_name {
                continue;
            }
            let ip = peer.ipv4.unwrap_or(std::net::Ipv4Addr::UNSPECIFIED);
            if let Some(our) = our_ip {
                if ip == our {
                    continue;
                }
            }
            let key = format!(
                "awdl:{}",
                peer.mac_address
                    .iter()
                    .map(|b| format!("{:02x}", b))
                    .collect::<String>()
            );
            by_key.entry(key).or_insert(crate::network::DiscoveredDevice {
                name: peer.device_name,
                address: std::net::IpAddr::V4(ip),
                port: 8770,
                service_type: crate::network::ServiceType::AirDrop,
                txt_records: HashMap::new(),
            });
        }

        let mut devices: Vec<_> = by_key.into_values().collect();
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
        devices
    }

    /// Simula il trasferimento di un file
    async fn simulate_file_transfer() -> f32 {
        for progress in (0..=100).step_by(10) {
            tokio::time::sleep(Duration::from_millis(200)).await;
            if progress == 100 {
                return 100.0;
            }
        }
        100.0
    }

    /// Aggiunge una notifica alla lista
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
        
        // Mantieni solo le ultime 5 notifiche
        if self.notifications.len() > 5 {
            self.notifications.remove(0);
        }
    }
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
        let _ = path;
        Err("Open folder is only supported on Windows".into())
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

/// Funzione principale per avviare l'applicazione
pub fn run(services: Arc<crate::AirDropdServices>) -> iced::Result {
    // Prefer DirectX 12 backend on Windows to avoid Vulkan validation spam
    // and disable extra WGPU validation layers in release usage.
    // These can be overridden by user environment variables if needed.
    std::env::set_var("WGPU_BACKEND", "dx12");
    std::env::set_var("WGPU_VALIDATION", "0");

    let window_icon = assets::load_window_icon();

    AirDropdApp::run(Settings {
        flags: services,
        id: None,
        fonts: Vec::new(),
        window: iced::window::Settings {
            size: iced::Size::new(680.0, 560.0),
            min_size: Some(iced::Size::new(560.0, 480.0)),
            position: iced::window::Position::Centered,
            resizable: true,
            decorations: true,
            transparent: false,
            icon: window_icon,
            ..Default::default()
        },
        default_font: iced::Font::DEFAULT,
        default_text_size: iced::Pixels(13.0),
        antialiasing: true,
    })
}

/// Avvia l'applicazione AirDropd con i servizi forniti
pub async fn run_app(
    _services: std::sync::Arc<crate::AirDropdServices>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Prefer DX12 and disable WGPU validation in async run path as well
    std::env::set_var("WGPU_BACKEND", "dx12");
    std::env::set_var("WGPU_VALIDATION", "0");

    let window_icon = assets::load_window_icon();

    AirDropdApp::run(Settings {
        flags: _services,
        id: None,
        fonts: Vec::new(),
        window: iced::window::Settings {
            size: iced::Size::new(680.0, 560.0),
            min_size: Some(iced::Size::new(560.0, 480.0)),
            position: iced::window::Position::Centered,
            resizable: true,
            decorations: true,
            transparent: false,
            icon: window_icon,
            ..Default::default()
        },
        antialiasing: true,
        default_font: iced::Font::DEFAULT,
        default_text_size: iced::Pixels(14.0),
    })?;
    Ok(())
}

/// Macro di utilità per creare elementi con spaziatura
#[macro_export]
macro_rules! spaced {
    ($spacing:expr, $($element:expr),+ $(,)?) => {
        iced::widget::column![$($element),+].spacing($spacing)
    };
}

/// Macro di utilità per creare righe con spaziatura
#[macro_export]
macro_rules! spaced_row {
    ($spacing:expr, $($element:expr),+ $(,)?) => {
        iced::widget::row![$($element),+].spacing($spacing)
    };
}
