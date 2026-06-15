//! Message definitions for the Iced application architecture.
//!
//! This module contains every message the application can send while
//! handling system events and user actions.

use crate::network::DiscoveredDevice;
use crate::protocols::airdrop::AirDropStatus;
use std::path::PathBuf;

/// Main application messages.
#[derive(Debug, Clone)]
pub enum Message {
    // System messages
    Tick,
    ThemeChanged(crate::ui::Theme),
    InitializationComplete,
    SplashTick,
    SplashComplete,
    SonarTick,
    CheckFirewall,
    FirewallPromptComplete(crate::network::firewall::FirewallPromptResult),
    
    // Discovery messages
    StartScanning,
    StopScanning,
    DevicesUpdated(Vec<DiscoveredDevice>),
    DevicesRefreshed(Vec<DiscoveredDevice>),
    RefreshDevices,
    DeviceSelected(DiscoveredDevice),
    DeviceDeselected,
    
    // AirDrop messages
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
    
    // Interface messages
    ShowLinkDialog,
    HideLinkDialog,
    LinkInputChanged(String),
    
    // Notification messages
    ShowNotification(NotificationMessage),
    HideNotification,
    
    // Error and info messages
    Error(String),
    Info(String),
    
    // Settings messages
    OpenLogFolder,
    OpenReceiveFolder,
    ClearCache,
    RunDiagnostics,
    
    // Navigation messages
    ShowMainView,
    ShowSettings,
    ShowAbout,
    CloseAbout,
    OpenCashAppDonation,

    // User settings
    BroadcastNameChanged(String),
    DownloadDirChanged(String),
    BrowseDownloadDir,
    DownloadDirSelected(Option<PathBuf>),
    MinimizeToTrayChanged(bool),
    AutoAcceptIncomingChanged(bool),
    ShowAllDevicesChanged(bool),
    SaveSettings,
    ResetSettings,

    // Live Activity panel
    ShowActivity,
    ClearActivityLog,

    // Web Drop (QR upload) screen
    ShowWebDrop,
    ShowDjMode,
    ExitDjMode,
    RefreshWebDropUrl,
    DjScanDrawers,
    DjDrawerOpen(String),
    DjDrawerRenameStart(String),
    DjDrawerRenameInput(String),
    DjDrawerRenameSubmit,
    DjDrawerRenameCancel,
    DjDrawerMoveUp(String),
    DjDrawerMoveDown(String),

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

    // Main view layout
    SetDeviceViewMode(crate::ui::views::device_list_view::DeviceViewMode),
    ListSortBy(crate::ui::views::device_list_view::ListSortColumn),
    ToggleDiscoveryFreeze,

    // Registration / licensing
    LicenseKeyInputChanged(String),
    ActivateLicense,
    DeactivateLicense,
    
    // External link messages
    OpenLicenses,
    OpenWebsite,
    OpenDocumentation,
    OpenIssues,
    OpenFeatureRequest,
}

/// Notification message payload.
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

/// Subscription messages for asynchronous events.
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