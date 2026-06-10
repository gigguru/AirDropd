//! Main view — macOS AirDrop-style layout.
//!
//! Interactive radar with devices placed by distance, drag-and-drop sending,
//! action sheet for the selected device, and modal overlays for incoming
//! transfers, link sending, and drop-recipient selection.

use iced::{
    widget::{
        button, column, container, pick_list, row, scrollable, text, text_input, Space,
    },
    Alignment, Element, Length, Theme as IcedTheme,
};

use crate::ui::{
    components,
    messages::{Message, NotificationMessage},
    radar,
    styles,
    widgets,
    Theme,
};
use crate::ui::views::settings_view::AirDropVisibility;

const VISIBILITY_OPTIONS: [AirDropVisibility; 3] = [
    AirDropVisibility::Everyone,
    AirDropVisibility::ContactsOnly,
    AirDropVisibility::ReceivingOff,
];

pub struct MainView<'a> {
    discovered_devices: &'a [crate::network::DiscoveredDevice],
    selected_device: Option<&'a crate::network::DiscoveredDevice>,
    is_scanning: bool,
    sonar_tick: u32,
    airdrop_status: &'a crate::protocols::airdrop::AirDropStatus,
    file_transfer_progress: Option<f32>,
    notifications: &'a [NotificationMessage],
    show_link_dialog: bool,
    link_url: &'a str,
    pending_incoming: Option<&'a crate::protocols::incoming_transfer::IncomingTransferDetails>,
    pending_recipient_files: Option<&'a [std::path::PathBuf]>,
    drop_hover: bool,
    visibility: AirDropVisibility,
}

#[allow(clippy::too_many_arguments)]
pub fn render<'a>(
    discovered_devices: &'a [crate::network::DiscoveredDevice],
    selected_device: Option<&'a crate::network::DiscoveredDevice>,
    is_scanning: bool,
    sonar_tick: u32,
    airdrop_status: &'a crate::protocols::airdrop::AirDropStatus,
    file_transfer_progress: Option<f32>,
    notifications: &'a [NotificationMessage],
    show_link_dialog: bool,
    link_url: &'a str,
    pending_incoming: Option<&'a crate::protocols::incoming_transfer::IncomingTransferDetails>,
    pending_recipient_files: Option<&'a [std::path::PathBuf]>,
    drop_hover: bool,
    visibility: AirDropVisibility,
    theme: &Theme,
) -> Element<'a, Message> {
    MainView {
        discovered_devices,
        selected_device,
        is_scanning,
        sonar_tick,
        airdrop_status,
        file_transfer_progress,
        notifications,
        show_link_dialog,
        link_url,
        pending_incoming,
        pending_recipient_files,
        drop_hover,
        visibility,
    }
    .view(theme)
}

/// Modal overlay for incoming AirDrop /Ask accept or reject.
pub fn incoming_transfer_overlay<'a>(
    incoming: &crate::protocols::incoming_transfer::IncomingTransferDetails,
) -> Element<'a, Message> {
    use crate::protocols::incoming_transfer::format_bytes;

    let file_lines: Element<'a, Message> = if let Some(link) = &incoming.link {
        text(link.clone())
            .size(12)
            .style(styles::colors::TEXT_SECONDARY)
            .into()
    } else if incoming.files.is_empty() {
        text("Incoming file transfer")
            .size(13)
            .style(styles::colors::TEXT_SECONDARY)
            .into()
    } else {
        let shown = incoming.files.iter().take(5);
        let extra = incoming.files.len().saturating_sub(5);
        let mut col = shown.fold(column![].spacing(4), |col, file| {
            col.push(
                text(format!("• {} ({})", file.name, format_bytes(file.size)))
                    .size(12)
                    .style(styles::colors::TEXT_MUTED),
            )
        });
        if extra > 0 {
            col = col.push(
                text(format!("…and {} more", extra))
                    .size(12)
                    .style(styles::colors::TEXT_MUTED),
            );
        }
        col.into()
    };

    let total = incoming.total_bytes();
    let size_line: Element<'a, Message> = if total > 0 {
        text(format!("Total size: {}", format_bytes(total)))
            .size(11)
            .style(styles::colors::TEXT_MUTED)
            .into()
    } else {
        Space::with_height(0).into()
    };

    let dialog = container(
        column![
            text("Accept AirDrop?")
                .size(18)
                .style(styles::colors::PRIMARY),
            Space::with_height(8),
            text(incoming.summary())
                .size(14)
                .style(styles::colors::TEXT_PRIMARY),
            text(format!("From: {}", incoming.sender_model))
                .size(12)
                .style(styles::colors::TEXT_MUTED),
            Space::with_height(8),
            file_lines,
            size_line,
            Space::with_height(16),
            row![
                button(text("Decline").size(14))
                    .on_press(Message::RejectIncomingTransfer)
                    .style(iced::theme::Button::Secondary)
                    .padding([8, 20]),
                Space::with_width(12),
                button(text("Accept").size(14))
                    .on_press(Message::AcceptIncomingTransfer)
                    .style(iced::theme::Button::Primary)
                    .padding([8, 20]),
            ]
            .align_items(Alignment::Center),
        ]
        .align_items(Alignment::Center)
        .padding(24),
    )
    .style(|_: &IcedTheme| container::Appearance {
        background: Some(iced::Background::Color(styles::colors::SURFACE)),
        border: iced::Border {
            color: styles::colors::PRIMARY,
            width: 1.5,
            radius: 12.0.into(),
        },
        shadow: iced::Shadow {
            color: iced::Color::from_rgba(0.0, 0.0, 0.0, 0.45),
            offset: iced::Vector::new(0.0, 8.0),
            blur_radius: 24.0,
        },
        ..Default::default()
    })
    .width(Length::Fixed(380.0));

    scrim(dialog.into())
}

/// Dim the whole window behind a centered dialog.
fn scrim(dialog: Element<'_, Message>) -> Element<'_, Message> {
    container(
        container(dialog)
            .center_x()
            .center_y()
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_: &IcedTheme| container::Appearance {
        background: Some(iced::Background::Color(iced::Color::from_rgba(
            0.0, 0.0, 0.0, 0.55,
        ))),
        ..Default::default()
    })
    .into()
}

impl<'a> MainView<'a> {
    pub fn view(&self, theme: &Theme) -> Element<'a, Message> {
        let iced_theme = match theme {
            Theme::Dark => IcedTheme::Dark,
            Theme::Light => IcedTheme::Light,
        };
        let bg = styles::background(*theme);

        let body = column![
            self.toolbar(theme),
            container(radar::radar(
                self.discovered_devices,
                self.selected_device,
                self.is_scanning,
                self.sonar_tick,
                &iced_theme,
                self.drop_hover,
            ))
            .width(Length::Fill)
            .height(Length::FillPortion(1)),
            self.status_line(theme),
            if self.selected_device.is_some() {
                self.action_sheet(theme)
            } else {
                Space::with_height(0).into()
            },
            if let Some(progress) = self.file_transfer_progress {
                self.transfer_progress(progress, theme)
            } else {
                Space::with_height(0).into()
            },
            self.discovery_bar(theme),
            components::copyright_footer(theme),
        ]
        .spacing(0)
        .width(Length::Fill)
        .height(Length::Fill);

        let content = container(body)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding([12, 20, 8, 20])
            .style(move |_: &IcedTheme| iced::widget::container::Appearance {
                background: Some(iced::Background::Color(bg)),
                ..Default::default()
            });

        if let Some(incoming) = self.pending_incoming {
            incoming_transfer_overlay(incoming)
        } else if let Some(files) = self.pending_recipient_files {
            self.recipient_chooser(files, &iced_theme, theme)
        } else if self.show_link_dialog {
            self.link_dialog(theme)
        } else {
            content.into()
        }
    }

    fn status_line(&self, theme: &Theme) -> Element<'a, Message> {
        let status_text = if self.drop_hover {
            "Release to send the files".to_string()
        } else if self.is_scanning {
            "Looking for others…".to_string()
        } else if self.discovered_devices.is_empty() {
            "No devices found — open AirDrop on your iPhone or Mac".to_string()
        } else {
            format!(
                "{} device{} nearby — tap a device or drop files on it",
                self.discovered_devices.len(),
                if self.discovered_devices.len() == 1 { "" } else { "s" }
            )
        };
        container(
            text(status_text)
                .size(13)
                .style(styles::text_color_muted(*theme))
                .horizontal_alignment(iced::alignment::Horizontal::Center),
        )
        .width(Length::Fill)
        .center_x()
        .padding([4, 0])
        .into()
    }

    fn toolbar(&self, theme: &Theme) -> Element<'a, Message> {
        let mut bar = row![
            Space::with_width(Length::Fixed(32.0)),
            text("AirDrop")
                .size(13)
                .style(styles::text_color_secondary(*theme)),
            Space::with_width(Length::Fill),
            button(
                text("⚙")
                    .size(16)
                    .style(styles::text_color_secondary(*theme))
            )
            .on_press(Message::ShowSettings)
            .style(iced::theme::Button::Text)
            .padding([4, 8]),
            button(
                text(if self.is_scanning { "Stop" } else { "Refresh" })
                    .size(12)
                    .style(styles::text_color_secondary(*theme))
            )
            .on_press(if self.is_scanning {
                Message::StopScanning
            } else {
                Message::StartScanning
            })
            .style(iced::theme::Button::Text)
            .padding([4, 8]),
        ]
        .align_items(Alignment::Center);

        if !self.notifications.is_empty() {
            if let Some(n) = self.notifications.last() {
                bar = bar.push(
                    text(&n.title)
                        .size(11)
                        .style(styles::text_color_muted(*theme)),
                );
            }
        }

        bar.into()
    }

    fn action_sheet(&self, theme: &Theme) -> Element<'a, Message> {
        let device = match self.selected_device {
            Some(d) => d,
            None => return Space::with_height(0).into(),
        };

        let surface = styles::surface(*theme);
        let idle = matches!(
            self.airdrop_status,
            crate::protocols::airdrop::AirDropStatus::Idle
                | crate::protocols::airdrop::AirDropStatus::Connected
        );
        let ble_only = device.address.is_unspecified() || device.port == 0;

        let hint: Element<'a, Message> = if ble_only {
            text("Visible via Bluetooth — waiting for Wi‑Fi to enable transfers")
                .size(11)
                .style(styles::text_color_muted(*theme))
                .into()
        } else {
            Space::with_height(0).into()
        };

        let actions = column![
            text(format!(
                "{} {} · {}",
                device.kind().emoji(),
                device.name,
                device.kind().label()
            ))
            .size(13)
            .style(styles::text_color(*theme)),
            Space::with_height(8),
            row![
                button(text("Send Files").size(12))
                    .on_press_maybe(
                        (idle && !ble_only).then(|| Message::SendFile(device.clone()))
                    )
                    .padding([6, 14]),
                Space::with_width(8),
                button(text("Send Folder").size(12))
                    .on_press_maybe(
                        (idle && !ble_only).then(|| Message::SendFolder(device.clone()))
                    )
                    .padding([6, 14]),
                Space::with_width(8),
                button(text("Send Link").size(12))
                    .on_press_maybe((idle && !ble_only).then(|| Message::ShowLinkDialog))
                    .padding([6, 14]),
            ]
            .align_items(Alignment::Center),
            Space::with_height(4),
            hint,
        ]
        .align_items(Alignment::Center)
        .spacing(4);

        container(actions)
            .padding(12)
            .width(Length::Fill)
            .center_x()
            .style(move |_: &IcedTheme| iced::widget::container::Appearance {
                background: Some(iced::Background::Color(surface)),
                border: iced::Border {
                    radius: 10.0.into(),
                    width: 1.0,
                    color: iced::Color::from_rgba(0.0, 0.0, 0.0, 0.08),
                },
                ..Default::default()
            })
            .into()
    }

    fn transfer_progress(&self, progress: f32, theme: &Theme) -> Element<'a, Message> {
        container(
            column![
                text("Transferring…")
                    .size(12)
                    .style(styles::text_color_secondary(*theme)),
                components::primary_progress_bar(progress),
                text(format!("{:.0}%", progress))
                    .size(11)
                    .style(styles::text_color_muted(*theme)),
            ]
            .align_items(Alignment::Center)
            .spacing(4)
            .padding(8),
        )
        .width(Length::Fill)
        .center_x()
        .into()
    }

    fn discovery_bar(&self, theme: &Theme) -> Element<'a, Message> {
        row![
            text("Allow me to be discovered by:")
                .size(12)
                .style(styles::text_color_secondary(*theme)),
            Space::with_width(8),
            pick_list(
                &VISIBILITY_OPTIONS[..],
                Some(self.visibility),
                Message::VisibilityChanged,
            )
            .text_size(12)
            .placeholder("Everyone"),
        ]
        .align_items(Alignment::Center)
        .padding([12, 0, 4, 0])
        .into()
    }

    /// Overlay shown after a drop when several devices could receive the files.
    fn recipient_chooser(
        &self,
        files: &'a [std::path::PathBuf],
        iced_theme: &IcedTheme,
        _theme: &Theme,
    ) -> Element<'a, Message> {
        let count = files.len();
        let label = if count == 1 {
            files[0]
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "1 item".to_string())
        } else {
            format!("{} items", count)
        };

        let device_rows: Element<'a, Message> = self
            .discovered_devices
            .iter()
            .cloned()
            .fold(column![].spacing(8).width(Length::Fill), |col, device| {
                let reachable = !device.address.is_unspecified() && device.port > 0;
                let kind = device.kind().label();
                let subtitle = if reachable {
                    format!("{} — Ready to receive", kind)
                } else {
                    format!("{} — Bluetooth only, not reachable yet", kind)
                };
                let icon = widgets::device_icon(&device);
                let msg = Message::ChooseRecipient(device.clone());
                col.push(widgets::device_list_row(
                    &device.name,
                    icon,
                    &subtitle,
                    false,
                    iced_theme,
                    msg,
                ))
            })
            .into();

        let dialog = container(
            column![
                text(format!("Send {} to…", label))
                    .size(16)
                    .style(styles::colors::TEXT_PRIMARY),
                Space::with_height(12),
                scrollable(device_rows).height(Length::Fixed(260.0)),
                Space::with_height(12),
                button(text("Cancel").size(13))
                    .on_press(Message::CancelRecipientChooser)
                    .style(iced::theme::Button::Secondary)
                    .padding([6, 18]),
            ]
            .align_items(Alignment::Center)
            .padding(20),
        )
        .style(|_: &IcedTheme| container::Appearance {
            background: Some(iced::Background::Color(styles::colors::SURFACE)),
            border: iced::Border {
                color: styles::colors::PRIMARY,
                width: 1.0,
                radius: 12.0.into(),
            },
            ..Default::default()
        })
        .width(Length::Fixed(400.0));

        scrim(dialog.into())
    }

    fn link_dialog(&self, theme: &Theme) -> Element<'a, Message> {
        let surface = styles::surface(*theme);

        let dialog = container(
            column![
                text("Send Link")
                    .size(15)
                    .style(styles::text_color(*theme)),
                Space::with_height(12),
                text_input("Enter URL…", self.link_url)
                    .on_input(Message::LinkInputChanged)
                    .width(Length::Fill)
                    .padding(8),
                Space::with_height(12),
                row![
                    button(text("Cancel").size(12))
                        .on_press(Message::HideLinkDialog)
                        .padding([6, 16]),
                    Space::with_width(Length::Fill),
                    button(text("Send").size(12))
                        .on_press_maybe(if !self.link_url.trim().is_empty() {
                            self.selected_device
                                .map(|d| Message::SendLink(d.clone(), self.link_url.to_string()))
                        } else {
                            None
                        })
                        .padding([6, 16]),
                ]
                .align_items(Alignment::Center),
            ]
            .spacing(4)
            .max_width(360),
        )
        .padding(20)
        .style(move |_: &IcedTheme| iced::widget::container::Appearance {
            background: Some(iced::Background::Color(surface)),
            border: iced::Border {
                radius: 12.0.into(),
                width: 1.0,
                color: iced::Color::from_rgba(0.0, 0.0, 0.0, 0.12),
            },
            shadow: iced::Shadow {
                color: iced::Color::from_rgba(0.0, 0.0, 0.0, 0.25),
                offset: iced::Vector::new(0.0, 4.0),
                blur_radius: 16.0,
            },
            ..Default::default()
        });

        scrim(dialog.into())
    }
}
