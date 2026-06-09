//! Main view — macOS AirDrop-style layout
//!
//! Centered radar, device bubbles, discovery visibility picker, and footer.

use iced::{
    widget::{
        button, column, container, pick_list, row, scrollable, text, text_input, Space,
    },
    Alignment, Element, Length, Theme as IcedTheme,
};

use crate::ui::{
    components,
    messages::{Message, NotificationMessage},
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
    airplay_status: &'a crate::protocols::airplay::AirPlayStatus,
    airdrop_status: &'a crate::protocols::airdrop::AirDropStatus,
    file_transfer_progress: Option<f32>,
    notifications: &'a [NotificationMessage],
    show_link_dialog: bool,
    link_url: &'a str,
    pending_incoming: Option<&'a crate::protocols::incoming_transfer::IncomingTransferDetails>,
    visibility: AirDropVisibility,
}

pub fn render<'a>(
    discovered_devices: &'a [crate::network::DiscoveredDevice],
    selected_device: Option<&'a crate::network::DiscoveredDevice>,
    is_scanning: bool,
    sonar_tick: u32,
    airplay_status: &'a crate::protocols::airplay::AirPlayStatus,
    airdrop_status: &'a crate::protocols::airdrop::AirDropStatus,
    file_transfer_progress: Option<f32>,
    notifications: &'a [NotificationMessage],
    show_link_dialog: bool,
    link_url: &'a str,
    pending_incoming: Option<&'a crate::protocols::incoming_transfer::IncomingTransferDetails>,
    visibility: AirDropVisibility,
    theme: &Theme,
) -> Element<'a, Message> {
    MainView::new(
        discovered_devices,
        selected_device,
        is_scanning,
        sonar_tick,
        airplay_status,
        airdrop_status,
        file_transfer_progress,
        notifications,
        show_link_dialog,
        link_url,
        pending_incoming,
        visibility,
    )
    .view(theme)
}

/// Modal overlay for incoming AirDrop /Ask accept or reject.
pub fn incoming_transfer_overlay<'a>(
    incoming: &crate::protocols::incoming_transfer::IncomingTransferDetails,
    _underlay: Element<'a, Message>,
) -> Element<'a, Message> {
    use crate::protocols::incoming_transfer::format_bytes;

    let file_lines: Element<'a, Message> = if incoming.files.is_empty() {
        text("Incoming file transfer")
            .size(13)
            .style(styles::colors::TEXT_SECONDARY)
            .into()
    } else {
        incoming
            .files
            .iter()
            .take(4)
            .fold(column![].spacing(4), |col, file| {
                col.push(
                    text(format!(
                        "• {} ({})",
                        file.name,
                        format_bytes(file.size)
                    ))
                    .size(12)
                    .style(styles::colors::TEXT_MUTED),
                )
            })
            .into()
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
        .padding(24)
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
    .width(Length::Fixed(360.0));

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
    pub fn new(
        discovered_devices: &'a [crate::network::DiscoveredDevice],
        selected_device: Option<&'a crate::network::DiscoveredDevice>,
        is_scanning: bool,
        sonar_tick: u32,
        airplay_status: &'a crate::protocols::airplay::AirPlayStatus,
        airdrop_status: &'a crate::protocols::airdrop::AirDropStatus,
        file_transfer_progress: Option<f32>,
        notifications: &'a [NotificationMessage],
    show_link_dialog: bool,
    link_url: &'a str,
    pending_incoming: Option<&'a crate::protocols::incoming_transfer::IncomingTransferDetails>,
    visibility: AirDropVisibility,
) -> Self {
        Self {
            discovered_devices,
            selected_device,
            is_scanning,
            sonar_tick,
            airplay_status,
            airdrop_status,
            file_transfer_progress,
            notifications,
            show_link_dialog,
            link_url,
            pending_incoming,
            visibility,
        }
    }

    pub fn view(&self, theme: &Theme) -> Element<'a, Message> {
        let iced_theme = match theme {
            Theme::Dark => IcedTheme::Dark,
            Theme::Light => IcedTheme::Light,
        };
        let bg = styles::background(*theme);

        let body = column![
            self.toolbar(theme),
            container(self.discovery_center(&iced_theme, theme))
                .width(Length::Fill)
                .height(Length::FillPortion(1))
                .align_y(iced::alignment::Vertical::Top)
                .padding([0, 4]),
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

        if self.show_link_dialog {
            container(column![content, self.link_dialog(theme)])
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        } else if let Some(incoming) = self.pending_incoming {
            incoming_transfer_overlay(incoming, content.into())
        } else {
            content.into()
        }
    }

    /// Center discovery zone: radar rings with a tappable device list beneath.
    fn discovery_center(
        &self,
        iced_theme: &IcedTheme,
        theme: &Theme,
    ) -> Element<'a, Message> {
        let status_text = if self.is_scanning {
            "Looking for others...".to_string()
        } else if self.discovered_devices.is_empty() {
            "No devices found — open AirDrop on your iPhone or Mac".to_string()
        } else {
            format!(
                "{} device{} nearby — select one below",
                self.discovered_devices.len(),
                if self.discovered_devices.len() == 1 { "" } else { "s" }
            )
        };

        column![
            text(status_text)
                .size(13)
                .style(styles::text_color_muted(*theme))
                .horizontal_alignment(iced::alignment::Horizontal::Center),
            Space::with_height(8),
            container(widgets::airdrop_radar(iced_theme, self.is_scanning, self.sonar_tick))
                .height(Length::Fixed(200.0))
                .center_x(),
            Space::with_height(12),
            self.device_list(iced_theme),
        ]
        .align_items(Alignment::Center)
        .width(Length::Fill)
        .spacing(0)
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

    #[allow(dead_code)]
    fn radar_area(
        &self,
        iced_theme: &IcedTheme,
        theme: &Theme,
    ) -> Element<'a, Message> {
        let status_text = if self.is_scanning {
            "Looking for others..."
        } else if self.discovered_devices.is_empty() {
            "No devices found"
        } else {
            "Nearby devices"
        };

        column![
            container(widgets::airdrop_radar(iced_theme, self.is_scanning, self.sonar_tick))
                .center_x()
                .height(Length::Fixed(240.0)),
            Space::with_height(8),
            text(status_text)
                .size(13)
                .style(styles::text_color_muted(*theme))
                .horizontal_alignment(iced::alignment::Horizontal::Center),
        ]
        .align_items(Alignment::Center)
        .width(Length::Fill)
        .into()
    }

    /// Scrollable list of nearby devices with full-width tap targets.
    fn device_list(&self, iced_theme: &IcedTheme) -> Element<'a, Message> {
        if self.discovered_devices.is_empty() {
            return container(
                text(if self.is_scanning {
                    "Searching for nearby Apple devices..."
                } else {
                    " "
                })
                .size(12)
                .style(styles::colors::TEXT_MUTED)
                .horizontal_alignment(iced::alignment::Horizontal::Center),
            )
            .height(Length::Fixed(80.0))
            .width(Length::Fill)
            .center_x()
            .center_y()
            .into();
        }

        let items: Element<'a, Message> = self
            .discovered_devices
            .iter()
            .cloned()
            .fold(column![].spacing(8).width(Length::Fill), |col, device| {
                let is_selected = self
                    .selected_device
                    .as_ref()
                    .map(|s| s.name == device.name && s.address == device.address)
                    .unwrap_or(false);
                let icon = widgets::device_icon(&device.service_type);
                let ble_only = device.port == 0 || device.address.is_unspecified();
                let subtitle = if ble_only {
                    "Bluetooth only — join the same Wi‑Fi to send files"
                } else {
                    match device.service_type {
                        crate::network::ServiceType::AirPlay | crate::network::ServiceType::Raop => {
                            "AirPlay device — tap for options"
                        }
                        crate::network::ServiceType::AirDrop
                        | crate::network::ServiceType::Companion => "Ready to receive files",
                        _ => "Tap to select",
                    }
                };
                let device_for_msg = device.clone();
                col.push(widgets::device_list_row(
                    &device.name,
                    icon,
                    subtitle,
                    is_selected,
                    iced_theme,
                    Message::DeviceSelected(device_for_msg),
                ))
            })
            .into();

        scrollable(
            container(items)
                .width(Length::Fill)
                .padding([0, 4]),
        )
        .height(Length::Fill)
        .width(Length::Fill)
        .into()
    }

    #[allow(dead_code)]
    fn device_orbit(&self, iced_theme: &IcedTheme) -> Element<'a, Message> {
        self.device_row(iced_theme)
    }

    fn device_row(&self, iced_theme: &IcedTheme) -> Element<'a, Message> {
        if self.discovered_devices.is_empty() {
            return container(
                text(if self.is_scanning { "Searching..." } else { " " })
                    .size(12)
                    .style(styles::colors::TEXT_MUTED),
            )
            .height(Length::Fixed(100.0))
            .center_x()
            .center_y()
            .into();
        }

        let bubbles: Element<'a, Message> = self
            .discovered_devices
            .iter()
            .cloned()
            .fold(
                row![].spacing(20).align_items(Alignment::Center),
                |row_el, device| {
                    let is_selected = self
                        .selected_device
                        .as_ref()
                        .map(|s| s.name == device.name && s.address == device.address)
                        .unwrap_or(false);
                    let icon = widgets::device_icon(&device.service_type);
                    let ble_only = device.port == 0 || device.address.is_unspecified();
                    let label = if ble_only {
                        format!("{} 📡", device.name)
                    } else {
                        device.name.clone()
                    };
                    let device_for_msg = device.clone();
                    row_el.push(widgets::device_bubble(
                        &label,
                        icon,
                        is_selected,
                        iced_theme,
                        Message::DeviceSelected(device_for_msg),
                    ))
                },
            )
            .into();

        scrollable(
            container(bubbles)
                .center_x()
                .width(Length::Fill)
                .padding([4, 8]),
        )
        .direction(iced::widget::scrollable::Direction::Horizontal(
            iced::widget::scrollable::Properties::default().scroller_width(6.0),
        ))
        .height(Length::Fixed(100.0))
        .width(Length::Fill)
        .into()
    }

    fn action_sheet(&self, theme: &Theme) -> Element<'a, Message> {
        let device = match self.selected_device {
            Some(d) => d,
            None => return Space::with_height(0).into(),
        };

        let surface = styles::surface(*theme);
        let is_airplay = matches!(
            device.service_type,
            crate::network::ServiceType::AirPlay
        );

        let mut actions = column![
            text(&device.name)
                .size(13)
                .style(styles::text_color(*theme)),
            Space::with_height(8),
            row![
                button(text("Send File").size(12))
                    .on_press_maybe(
                        if matches!(
                            self.airdrop_status,
                            crate::protocols::airdrop::AirDropStatus::Idle
                                | crate::protocols::airdrop::AirDropStatus::Connected
                        ) {
                            Some(Message::SendFile(device.clone()))
                        } else {
                            None
                        }
                    )
                    .padding([6, 14]),
                Space::with_width(8),
                button(text("Send Link").size(12))
                    .on_press_maybe(
                        if matches!(
                            self.airdrop_status,
                            crate::protocols::airdrop::AirDropStatus::Idle
                                | crate::protocols::airdrop::AirDropStatus::Connected
                        ) {
                            Some(Message::ShowLinkDialog)
                        } else {
                            None
                        }
                    )
                    .padding([6, 14]),
            ]
            .align_items(Alignment::Center),
        ]
        .align_items(Alignment::Center)
        .spacing(4);

        if is_airplay {
            let (label, action) = match self.airplay_status {
                crate::protocols::airplay::AirPlayStatus::Idle => {
                    ("Connect AirPlay", Some(Message::StartScreenMirroring(device.clone())))
                }
                crate::protocols::airplay::AirPlayStatus::Connecting => {
                    ("Connecting...", None)
                }
                crate::protocols::airplay::AirPlayStatus::Connected => {
                    ("Disconnect", Some(Message::StopScreenMirroring))
                }
                crate::protocols::airplay::AirPlayStatus::Failed(_) => {
                    ("Retry AirPlay", Some(Message::StartScreenMirroring(device.clone())))
                }
            };
            actions = actions.push(Space::with_height(4));
            actions = actions.push(
                button(text(label).size(12))
                    .on_press_maybe(action)
                    .padding([6, 14]),
            );
        }

        container(actions)
            .padding(12)
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
                text("Transferring...")
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

    fn notifications_overlay(&self, theme: &Theme) -> Element<'a, Message> {
        let notifications: Element<Message> = self
            .notifications
            .iter()
            .fold(column![].spacing(4), |col, notification| {
                col.push(
                    container(column![
                        text(&notification.title)
                            .size(12)
                            .style(styles::text_color(*theme)),
                        text(&notification.content)
                            .size(11)
                            .style(styles::text_color_secondary(*theme)),
                    ])
                    .padding(8)
                    .style(|_: &IcedTheme| iced::widget::container::Appearance {
                        background: Some(iced::Background::Color(styles::colors::SURFACE)),
                        border: iced::Border {
                            radius: 8.0.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    }),
                )
            })
            .into();

        container(notifications)
            .padding([0, 20, 8, 20])
            .into()
    }

    fn link_dialog(&self, theme: &Theme) -> Element<'a, Message> {
        let surface = styles::surface(*theme);

        let dialog = container(
            column![
                text("Send Link")
                    .size(15)
                    .style(styles::text_color(*theme)),
                Space::with_height(12),
                text_input("Enter URL...", self.link_url)
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

        container(dialog)
            .center_x()
            .center_y()
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_: &IcedTheme| iced::widget::container::Appearance {
                background: Some(iced::Background::Color(iced::Color::from_rgba(
                    0.0, 0.0, 0.0, 0.4,
                ))),
                ..Default::default()
            })
            .into()
    }
}
