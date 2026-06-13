//! Settings view — every control here is wired to real behavior.

use iced::{
    widget::{
        button, checkbox, column, container, row, scrollable, text, text_input, Space,
        horizontal_rule,
    },
    Alignment, Element, Length,
};

use crate::ui::{messages::Message, styles, Theme};
use crate::config::{AppConfig, DiscoveryMode};
use std::path::PathBuf;

/// Discovery visibility options (shared with the main view picker).
pub type AirDropVisibility = DiscoveryMode;

/// Persistent state for the settings form.
#[derive(Debug, Clone)]
pub struct SettingsView {
    broadcast_name: String,
    download_dir_text: String,
    minimize_to_tray: bool,
    auto_accept_incoming: bool,
    show_all_devices: bool,
}

impl SettingsView {
    pub fn from_config(cfg: &AppConfig) -> Self {
        Self {
            broadcast_name: cfg.broadcast_name.clone(),
            download_dir_text: cfg.download_dir.display().to_string(),
            minimize_to_tray: cfg.minimize_to_tray,
            auto_accept_incoming: cfg.auto_accept_incoming,
            show_all_devices: cfg.show_all_devices,
        }
    }

    pub fn apply_to_config(&self, cfg: &mut AppConfig) {
        cfg.broadcast_name = self.broadcast_name.trim().to_string();
        if cfg.broadcast_name.is_empty() {
            cfg.broadcast_name = crate::config::default_broadcast_name();
        }
        cfg.download_dir = PathBuf::from(self.download_dir_text.trim());
        if cfg.download_dir.as_os_str().is_empty() {
            cfg.download_dir = crate::config::default_download_dir();
        }
        cfg.minimize_to_tray = self.minimize_to_tray;
        cfg.auto_accept_incoming = self.auto_accept_incoming;
        cfg.show_all_devices = self.show_all_devices;
    }

    pub fn set_broadcast_name(&mut self, name: String) {
        self.broadcast_name = name;
    }

    pub fn set_download_dir_text(&mut self, path: String) {
        self.download_dir_text = path;
    }

    pub fn set_minimize_to_tray(&mut self, value: bool) {
        self.minimize_to_tray = value;
    }

    pub fn set_auto_accept_incoming(&mut self, value: bool) {
        self.auto_accept_incoming = value;
    }

    pub fn set_show_all_devices(&mut self, value: bool) {
        self.show_all_devices = value;
    }

    pub fn view(&self, theme: &Theme) -> Element<Message> {
        let header = row![
            button(text("← Back").size(14))
                .on_press(Message::ShowMainView)
                .style(iced::theme::Button::Secondary),
            Space::with_width(styles::spacing::MEDIUM),
            text("Settings").size(24),
            Space::with_width(Length::Fill),
            button(text("Save").size(14))
                .on_press(Message::SaveSettings)
                .style(iced::theme::Button::Primary),
            Space::with_width(styles::spacing::SMALL),
            button(text("Reset").size(14))
                .on_press(Message::ResetSettings)
                .style(iced::theme::Button::Secondary),
        ]
        .align_items(Alignment::Center)
        .padding(styles::spacing::MEDIUM.0);

        let content = scrollable(
            column![
                self.general_settings(theme),
                Space::with_height(styles::spacing::LARGE),
                self.airdrop_settings(theme),
                Space::with_height(styles::spacing::LARGE),
                self.maintenance_settings(theme),
                Space::with_height(styles::spacing::LARGE),
            ]
            .spacing(0),
        )
        .height(Length::Fill);

        container(column![header, horizontal_rule(1), content])
            .padding(styles::spacing::MEDIUM.0)
            .into()
    }

    fn general_settings(&self, _theme: &Theme) -> Element<Message> {
        let settings = column![
            row![
                text("Broadcast name:")
                    .size(14)
                    .width(Length::FillPortion(1)),
                text_input("Computer name", &self.broadcast_name)
                    .on_input(Message::BroadcastNameChanged)
                    .width(Length::FillPortion(2))
                    .padding(6),
            ]
            .align_items(Alignment::Center)
            .spacing(styles::spacing::MEDIUM),
            text("This name appears when others share files to you.")
                .size(12),
            row![
                text("Save folder:")
                    .size(14)
                    .width(Length::FillPortion(1)),
                text_input("Downloads", &self.download_dir_text)
                    .on_input(Message::DownloadDirChanged)
                    .width(Length::FillPortion(2))
                    .padding(6),
                button(text("Browse").size(13))
                    .on_press(Message::BrowseDownloadDir)
                    .padding([6, 12]),
            ]
            .align_items(Alignment::Center)
            .spacing(styles::spacing::MEDIUM),
            text(format!(
                "Transfers go to {} (QR uploads use the WebDrop subfolder).",
                AppConfig::default_save_paths_hint()
            ))
            .size(12),
            checkbox("Minimize to system tray", self.minimize_to_tray)
                .on_toggle(Message::MinimizeToTrayChanged),
        ]
        .spacing(styles::spacing::MEDIUM);

        section("General", settings.into())
    }

    fn airdrop_settings(&self, _theme: &Theme) -> Element<Message> {
        let settings = column![
            checkbox(
                "Automatically accept incoming transfers",
                self.auto_accept_incoming,
            )
            .on_toggle(Message::AutoAcceptIncomingChanged),
            text(
                "When enabled, files sent to this PC are saved without asking. \
                 Great for performances: guests' tracks land straight in your download folder.",
            )
            .size(12),
            checkbox("Show all nearby devices", self.show_all_devices)
                .on_toggle(Message::ShowAllDevicesChanged),
            text(
                "Also shows headphones, trackers, watches, and other nearby beacons on the \
                 radar — walk around and watch the distance update to locate a lost device.",
            )
            .size(12),
        ]
        .spacing(styles::spacing::MEDIUM);

        section("AirDrop", settings.into())
    }

    fn maintenance_settings(&self, _theme: &Theme) -> Element<Message> {
        let settings = column![row![
            button(text("Open Save Folder").size(14))
                .on_press(Message::OpenReceiveFolder)
                .style(iced::theme::Button::Secondary),
            button(text("Open Data Folder").size(14))
                .on_press(Message::OpenLogFolder)
                .style(iced::theme::Button::Secondary),
            button(text("🧹 Clear Cache").size(14))
                .on_press(Message::ClearCache)
                .style(iced::theme::Button::Secondary),
            button(text("📊 Diagnostics").size(14))
                .on_press(Message::RunDiagnostics)
                .style(iced::theme::Button::Secondary),
        ]
        .spacing(styles::spacing::MEDIUM)]
        .spacing(styles::spacing::MEDIUM);

        section("Maintenance", settings.into())
    }
}

fn section<'a>(title: &'a str, body: Element<'a, Message>) -> Element<'a, Message> {
    container(column![
        text(title).size(18),
        Space::with_height(styles::spacing::MEDIUM),
        body,
    ])
    .padding(styles::spacing::MEDIUM.0)
    .width(Length::Fill)
    .into()
}
