//! Definizione dei messaggi per l'architettura Iced
//! 
//! Questo modulo contiene tutti i messaggi che possono essere inviati
//! nell'applicazione per gestire gli eventi e le azioni dell'utente.

use crate::network::DiscoveredDevice;
use crate::protocols::airdrop::AirDropStatus;
use std::path::PathBuf;

/// Messaggi principali dell'applicazione
#[derive(Debug, Clone)]
pub enum Message {
    // Messaggi di sistema
    Tick,
    ThemeChanged(crate::ui::Theme),
    InitializationComplete,
    SplashTick,
    SplashComplete,
    SonarTick,
    CheckFirewall,
    FirewallPromptComplete(crate::network::firewall::FirewallPromptResult),
    
    // Messaggi di discovery
    StartScanning,
    StopScanning,
    DevicesUpdated(Vec<DiscoveredDevice>),
    DevicesRefreshed(Vec<DiscoveredDevice>),
    RefreshDevices,
    DeviceSelected(DiscoveredDevice),
    DeviceDeselected,
    
    // Messaggi di AirDrop
    AirDropStatusChanged(AirDropStatus),
    SendFile(DiscoveredDevice),
    SendFolder(DiscoveredDevice),
    SendLink(DiscoveredDevice, String),
    FileSendProgress(f32),
    FileSendCompleted(Result<(), String>),

    // Drag & drop of files/folders onto the window
    FileDroppedOnWindow(PathBuf),
    FilesHoveringWindow(bool),
    ProcessDroppedFiles(u64),
    ChooseRecipient(DiscoveredDevice),
    ChooseRecipientWithFiles(DiscoveredDevice, Vec<PathBuf>),
    CancelRecipientChooser,
    
    // Messaggi di interfaccia
    ShowLinkDialog,
    HideLinkDialog,
    LinkInputChanged(String),
    
    // Messaggi di notifica
    ShowNotification(NotificationMessage),
    HideNotification,
    
    // Messaggi di errore
    Error(String),
    Info(String),
    
    // Messaggi per le impostazioni
    OpenLogFolder,
    ClearCache,
    RunDiagnostics,
    
    // Messaggi per la navigazione
    ShowMainView,
    ShowSettings,
    ShowAbout,

    // User settings
    BroadcastNameChanged(String),
    DownloadDirChanged(String),
    BrowseDownloadDir,
    DownloadDirSelected(Option<PathBuf>),
    MinimizeToTrayChanged(bool),
    AutoAcceptIncomingChanged(bool),
    SaveSettings,
    ResetSettings,

    // System tray / window
    TrayAction(String),
    WindowCloseRequested,
    WindowMinimized,
    ShowWindow,
    FileReceived(PathBuf),
    QuitApp,
    PollTray,
    PollReceived,

    // Incoming AirDrop transfer prompt (/Ask)
    PollIncomingTransfer,
    UpdatePendingIncoming(Option<crate::protocols::incoming_transfer::IncomingTransferDetails>),
    AcceptIncomingTransfer,
    RejectIncomingTransfer,

    // Discovery visibility (macOS AirDrop-style)
    VisibilityChanged(crate::ui::views::settings_view::AirDropVisibility),
    
    // Messaggi per i link esterni
    OpenLicenses,
    OpenWebsite,
    OpenDocumentation,
    OpenIssues,
    OpenFeatureRequest,
}

/// Tipi di notifiche
#[derive(Debug, Clone)]
pub struct NotificationMessage {
    pub title: String,
    pub content: String,
    pub notification_type: NotificationType,
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NotificationType {
    Success,
    Warning,
    Error,
    Info,
}

impl NotificationMessage {
    pub fn success(title: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            content: content.into(),
            notification_type: NotificationType::Success,
            duration_ms: Some(3000),
        }
    }
    
    pub fn error(title: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            content: content.into(),
            notification_type: NotificationType::Error,
            duration_ms: Some(5000),
        }
    }
    
    pub fn warning(title: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            content: content.into(),
            notification_type: NotificationType::Warning,
            duration_ms: Some(4000),
        }
    }
    
    pub fn info(title: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            content: content.into(),
            notification_type: NotificationType::Info,
            duration_ms: Some(3000),
        }
    }
}

/// Subscription messages per eventi asincroni
#[derive(Debug, Clone)]
pub enum SubscriptionMessage {
    DeviceDiscoveryUpdate(Vec<DiscoveredDevice>),
    AirDropStatusUpdate(AirDropStatus),
    FileTransferProgress(f32),
}

impl From<SubscriptionMessage> for Message {
    fn from(sub_msg: SubscriptionMessage) -> Self {
        match sub_msg {
            SubscriptionMessage::DeviceDiscoveryUpdate(devices) => Message::DevicesUpdated(devices),
            SubscriptionMessage::AirDropStatusUpdate(status) => Message::AirDropStatusChanged(status),
            SubscriptionMessage::FileTransferProgress(progress) => Message::FileSendProgress(progress),
        }
    }
}