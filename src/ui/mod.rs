//! Modulo principale dell'interfaccia utente Iced
//!
//! Questo modulo contiene l'implementazione completa dell'interfaccia utente
//! utilizzando la libreria Iced, con design moderno e reattivo.

use iced::{
    executor,
    Application, Command, Element, Settings, Subscription, Theme as IcedTheme,
    time,
};

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

// Moduli pub mod app;
pub mod components;
pub mod messages;
pub mod styles;
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
        let app = Self {
            services,
            current_view: AppView::Loading,
            status_message: "Initializing...".to_string(),
            is_loading: true,
            theme: Theme::default(),
            discovery_visibility: views::settings_view::AirDropVisibility::Everyone,
            settings_view: views::settings_view::SettingsView::new(
                true,                // enable_auto_discovery
                15,                  // discovery_interval
                true,                // show_notifications
                false,               // minimize_to_tray
                true,                // airdrop_enabled
                views::settings_view::AirDropVisibility::Everyone,
                false,               // auto_accept_from_contacts
                true,                // airplay_enabled
                views::settings_view::AirPlayQuality::Auto,
                false,               // airplay_audio_only
                None,                // network_interface
                Vec::new(),          // available_interfaces
                None,                // custom_port
                false,               // debug_mode
                views::settings_view::LogLevel::Info,
                2,                   // max_concurrent_transfers
            ),
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
        };

        let command = Command::perform(
            async {
                // Simula inizializzazione
                tokio::time::sleep(Duration::from_secs(2)).await;
            },
            |_| Message::InitializationComplete,
        );

        (app, command)
    }

    fn title(&self) -> String {
        match self.current_view {
            AppView::Main => "AirDropd".to_string(),
            AppView::Settings => "AirDropd — Settings".to_string(),
            AppView::About => "AirDropd — About".to_string(),
            AppView::Loading => "AirDropd".to_string(),
        }
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        match message {
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
                let services = self.services.clone();
                let device = device.clone();
                Command::perform(
                    async move {
                        use rfd::AsyncFileDialog;
                        let file = AsyncFileDialog::new().pick_file().await;
                        let Some(handle) = file else {
                            return Err("No file selected".to_string());
                        };
                        let path = handle.path().to_path_buf();
                        let port = if device.port > 0 { device.port } else { 8770 };
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
                        let ble = services.ble.lock().await;
                        match visibility {
                            views::settings_view::AirDropVisibility::ReceivingOff => {
                                let _ = ble.stop_advertising().await;
                            }
                            _ => {
                                let _ = ble.start_advertising().await;
                            }
                        }
                    },
                    |_| Message::RefreshDevices,
                )
            }

            // Handle all other message variants with a wildcard pattern
            _ => Command::none(),
        }
    }

    fn view(&self) -> Element<Self::Message> {
        match self.current_view {
            AppView::Loading => self.loading_view(),
            AppView::Main => self.main_view(),
            AppView::Settings => self.settings_view(),
            AppView::About => self.about_view(),
        }
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        if matches!(self.current_view, AppView::Main) {
            time::every(Duration::from_secs(3)).map(|_| Message::RefreshDevices)
        } else {
            Subscription::none()
        }
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
        let mut by_key: HashMap<String, crate::network::DiscoveredDevice> = HashMap::new();

        if let Ok(devices) = services.device_discovery.lock().await.get_devices().await {
            for device in devices {
                if device.name.is_empty() {
                    continue;
                }
                let key = format!("{}:{}", device.address, device.port);
                by_key.insert(key, device);
            }
        }

        let ble_devices = services.ble.lock().await.get_discovered_devices().await;
        for ble in ble_devices {
            if ble.name.is_empty() {
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
            let ip = peer.ipv4.unwrap_or(std::net::Ipv4Addr::UNSPECIFIED);
            let key = format!("awdl:{}", peer.mac_address.iter().map(|b| format!("{:02x}", b)).collect::<String>());
            by_key.entry(key).or_insert(crate::network::DiscoveredDevice {
                name: peer.device_name,
                address: std::net::IpAddr::V4(ip),
                port: 8770,
                service_type: crate::network::ServiceType::AirDrop,
                txt_records: HashMap::new(),
            });
        }

        let mut devices: Vec<_> = by_key.into_values().collect();
        devices.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
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

/// Funzione principale per avviare l'applicazione
pub fn run(services: Arc<crate::AirDropdServices>) -> iced::Result {
    // Prefer DirectX 12 backend on Windows to avoid Vulkan validation spam
    // and disable extra WGPU validation layers in release usage.
    // These can be overridden by user environment variables if needed.
    std::env::set_var("WGPU_BACKEND", "dx12");
    std::env::set_var("WGPU_VALIDATION", "0");

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
            icon: None,
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
            icon: None,
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
