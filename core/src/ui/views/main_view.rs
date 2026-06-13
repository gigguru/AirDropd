//! Main view — sonar radar or sortable device list, with a device detail modal.

use iced::{
    widget::{
        button, column, container, horizontal_rule, image, pick_list, row, scrollable, svg,
        text, text_input, Space,
    },
    Alignment, Element, Length, Theme as IcedTheme,
};

use crate::ui::{
    assets,
    components,
    distance,
    icons,
    messages::{Message, NotificationMessage},
    radar,
    styles,
    widgets,
    Theme,
};
use crate::ui::views::{device_form, device_list_view};
use crate::ui::views::device_list_view::{DeviceViewMode, ListSortColumn};
use crate::ui::views::settings_view::AirDropVisibility;

const VISIBILITY_OPTIONS: [AirDropVisibility; 6] = [
    AirDropVisibility::Everyone,
    AirDropVisibility::ContactsOnly,
    AirDropVisibility::ReceivingOff,
    AirDropVisibility::AppleDevices,
    AirDropVisibility::AndroidDevices,
    AirDropVisibility::AirTags,
];

pub struct MainView<'a> {
    discovered_devices: &'a [crate::network::DiscoveredDevice],
    selected_device: Option<&'a crate::network::DiscoveredDevice>,
    device_view_mode: DeviceViewMode,
    list_sort_column: ListSortColumn,
    list_sort_ascending: bool,
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
    discovery_frozen: bool,
}

#[allow(clippy::too_many_arguments)]
pub fn render<'a>(
    discovered_devices: &'a [crate::network::DiscoveredDevice],
    selected_device: Option<&'a crate::network::DiscoveredDevice>,
    device_view_mode: DeviceViewMode,
    list_sort_column: ListSortColumn,
    list_sort_ascending: bool,
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
    discovery_frozen: bool,
    theme: &Theme,
) -> Element<'a, Message> {
    MainView {
        discovered_devices,
        selected_device,
        device_view_mode,
        list_sort_column,
        list_sort_ascending,
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
        discovery_frozen,
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
pub fn scrim(dialog: Element<'_, Message>) -> Element<'_, Message> {
    dismissible_scrim(dialog, None)
}

/// Modal scrim with optional click-outside-to-dismiss on the backdrop only.
pub fn dismissible_scrim<'a>(
    dialog: Element<'a, Message>,
    on_dismiss: Option<Message>,
) -> Element<'a, Message> {
    use iced::widget::{mouse_area, Space};

    let backdrop = |width: Length, height: Length| -> Element<'a, Message> {
        let panel = container(Space::new(width, height))
            .width(width)
            .height(height);
        if let Some(msg) = on_dismiss.clone() {
            mouse_area(panel).on_press(msg).into()
        } else {
            panel.into()
        }
    };

    container(
        column![
            backdrop(Length::Fill, Length::FillPortion(1)),
            row![
                backdrop(Length::FillPortion(1), Length::Shrink),
                dialog,
                backdrop(Length::FillPortion(1), Length::Shrink),
            ]
            .align_items(Alignment::Center),
            backdrop(Length::Fill, Length::FillPortion(1)),
        ]
        .width(Length::Fill)
        .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_: &IcedTheme| container::Appearance {
        background: Some(iced::Background::Color(iced::Color::from_rgba(
            0.0, 0.0, 0.0, 0.48,
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

        let device_area: Element<'a, Message> = match self.device_view_mode {
            DeviceViewMode::Sonar => container(radar::radar(
                self.discovered_devices,
                self.selected_device,
                !self.discovery_frozen,
                self.sonar_tick,
                &iced_theme,
                self.drop_hover,
            ))
            .width(Length::Fill)
            .height(Length::FillPortion(1))
            .into(),
            DeviceViewMode::List => device_list_view::render(
                self.discovered_devices,
                self.selected_device,
                self.list_sort_column,
                self.list_sort_ascending,
                self.discovery_frozen,
                theme,
            ),
        };

        let body = column![
            self.toolbar(theme),
            device_area,
            self.footer(theme),
        ]
        .spacing(0)
        .width(Length::Fill)
        .height(Length::Fill);

        let content = container(body)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding([6, 18, 6, 18])
            .style(move |_: &IcedTheme| iced::widget::container::Appearance {
                background: Some(iced::Background::Color(bg)),
                ..Default::default()
            });

        if let Some(incoming) = self.pending_incoming {
            incoming_transfer_overlay(incoming)
        } else if let Some(files) = self.pending_recipient_files {
            self.recipient_chooser(files, &iced_theme, theme)
        } else {
            let base: Element<'a, Message> = content.into();
            let with_device = if let Some(device) = self.selected_device {
                device_form::overlay(
                    device,
                    self.drop_hover,
                    self.file_transfer_progress,
                    self.airdrop_status,
                    theme,
                )
            } else {
                base
            };

            if self.show_link_dialog {
                self.link_dialog(theme)
            } else {
                with_device
            }
        }
    }

    fn footer(&self, theme: &Theme) -> Element<'a, Message> {
        column![
            self.status_line(theme),
            Space::with_height(Length::Fixed(4.0)),
            row![
                self.discovery_bar(theme),
                Space::with_width(Length::Fill),
            ]
            .align_items(Alignment::Center)
            .width(Length::Fill),
            components::copyright_footer(theme),
        ]
        .spacing(0)
        .width(Length::Fill)
        .into()
    }

    fn status_line(&self, theme: &Theme) -> Element<'a, Message> {
        let filter_hint = match self.visibility.device_filter() {
            crate::config::DeviceFilter::All => String::new(),
            crate::config::DeviceFilter::Apple => " — peer devices only".to_string(),
            crate::config::DeviceFilter::Android => " — Android devices only".to_string(),
            crate::config::DeviceFilter::AirTags => {
                " — trackers only (distance updates as you move)".to_string()
            }
        };
        let status_text = if self.discovery_frozen {
            "Discovery frozen — device list and sonar sweep paused".to_string()
        } else if self.drop_hover && self.selected_device.is_some() {
            "Release to send the files".to_string()
        } else if self.drop_hover {
            "Release to send the files".to_string()
        } else if self.is_scanning {
            format!("Looking for others…{filter_hint}")
        } else if self.discovered_devices.is_empty() {
            if self.visibility == AirDropVisibility::AirTags {
                "No trackers nearby — enable “Show all nearby devices” in Settings if needed"
                    .to_string()
            } else if self.visibility == AirDropVisibility::AndroidDevices {
                "No Android devices found nearby".to_string()
            } else if self.visibility == AirDropVisibility::AppleDevices {
                "No peer devices found — ask others to turn on sharing".to_string()
            } else if self.visibility == AirDropVisibility::ReceivingOff {
                "Receiving off — you won't appear in others' share lists".to_string()
            } else {
                "No devices found — ask others to turn on sharing".to_string()
            }
        } else {
            let mode_hint = match self.device_view_mode {
                DeviceViewMode::Sonar => "tap a device on the sonar",
                DeviceViewMode::List => "tap a row in the list",
            };
            let closest = self
                .discovered_devices
                .iter()
                .filter_map(|d| d.rssi.map(crate::ui::distance::rssi_to_feet))
                .min();
            let distance_hint = closest
                .map(|feet| format!(" — closest ~{}", distance::format_feet(feet)))
                .unwrap_or_default();
            format!(
                "{} device{} nearby — {}{}{}",
                self.discovered_devices.len(),
                if self.discovered_devices.len() == 1 { "" } else { "s" },
                mode_hint,
                distance_hint,
                filter_hint
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
        .padding([8, 0, 2, 0])
        .into()
    }

    fn toolbar_nav_tab(
        &self,
        label: &'static str,
        mode: DeviceViewMode,
        theme: &Theme,
    ) -> Element<'a, Message> {
        let active = self.device_view_mode == mode;
        button(
            text(label)
                .size(13)
                .style(if active {
                    iced::Color::WHITE
                } else {
                    styles::text_color(*theme)
                }),
        )
        .on_press(Message::SetDeviceViewMode(mode))
        .style(if active {
            iced::theme::Button::Primary
        } else {
            iced::theme::Button::Text
        })
        .padding([6, 14])
        .into()
    }

    fn toolbar_nav_link(
        &self,
        label: &'static str,
        message: Message,
        theme: &Theme,
        accent: Option<iced::Color>,
    ) -> Element<'a, Message> {
        button(
            text(label)
                .size(13)
                .style(accent.unwrap_or_else(|| styles::text_color(*theme))),
        )
        .on_press(message)
        .style(iced::theme::Button::Text)
        .padding([6, 10])
        .into()
    }

    fn toolbar_icon_button(
        svg_handle: iced::widget::svg::Handle,
        message: Message,
    ) -> Element<'static, Message> {
        button(svg(svg_handle).width(18).height(18))
            .on_press(message)
            .style(iced::theme::Button::Text)
            .padding([6, 8])
            .into()
    }

    fn freeze_control(&self, _theme: &Theme) -> Element<'static, Message> {
        const ICE: iced::Color = iced::Color::from_rgb(0.58, 0.84, 0.98);
        button(
            text("Freeze")
                .size(13)
                .style(if self.discovery_frozen {
                    iced::Color::WHITE
                } else {
                    ICE
                }),
        )
        .on_press(Message::ToggleDiscoveryFreeze)
        .style(if self.discovery_frozen {
            iced::theme::Button::Primary
        } else {
            iced::theme::Button::Text
        })
        .padding([6, 10])
        .into()
    }

    fn toolbar(&self, theme: &Theme) -> Element<'a, Message> {
        let logo = container(
            image(assets::toolbar_logo())
                .height(Length::Fixed(34.0))
                .width(Length::Fixed(34.0)),
        )
        .center_y()
        .padding([0, 2]);

        let mut bar = row![
            logo,
            Space::with_width(Length::Fixed(10.0)),
            self.toolbar_nav_tab("Sonar", DeviceViewMode::Sonar, theme),
            self.toolbar_nav_tab("List", DeviceViewMode::List, theme),
            self.freeze_control(theme),
            self.toolbar_nav_link("DJ Mode", Message::ShowDjMode, theme, None),
            self.toolbar_nav_link("Receive via QR", Message::ShowWebDrop, theme, None),
            self.toolbar_nav_link("Activity", Message::ShowActivity, theme, None),
            Space::with_width(Length::Fill),
            Self::toolbar_icon_button(icons::folder(), Message::OpenReceiveFolder),
            Self::toolbar_icon_button(icons::settings(), Message::ShowSettings),
            self.toolbar_nav_link(
                if self.is_scanning { "Stop" } else { "Refresh" },
                if self.is_scanning {
                    Message::StopScanning
                } else {
                    Message::StartScanning
                },
                theme,
                None,
            ),
            self.toolbar_nav_link("About", Message::ShowAbout, theme, None),
        ]
        .align_items(Alignment::Center)
        .spacing(2);

        if !self.notifications.is_empty() {
            if let Some(n) = self.notifications.last() {
                bar = bar.push(
                    text(&n.title)
                        .size(11)
                        .style(styles::text_color_muted(*theme)),
                );
            }
        }

        container(
            column![
                bar,
                horizontal_rule(1),
            ]
            .spacing(8),
        )
            .width(Length::Fill)
            .padding([4, 0, 6, 0])
            .into()
    }

    fn discovery_bar(&self, theme: &Theme) -> Element<'a, Message> {
        let is_dark = *theme == Theme::Dark;
        let dropdown_bg = if is_dark {
            iced::Color::from_rgb(0.20, 0.20, 0.22)
        } else {
            iced::Color::from_rgb(0.92, 0.92, 0.94)
        };

        row![
            text("Discovery:")
                .size(12)
                .style(styles::text_color_secondary(*theme)),
            Space::with_width(Length::Fixed(8.0)),
            container(
                pick_list(
                    &VISIBILITY_OPTIONS[..],
                    Some(self.visibility),
                    Message::VisibilityChanged,
                )
                .text_size(12)
                .placeholder("Everyone"),
            )
            .padding([4, 10])
            .style(move |_: &IcedTheme| iced::widget::container::Appearance {
                background: Some(iced::Background::Color(dropdown_bg)),
                border: iced::Border {
                    color: iced::Color::from_rgba(1.0, 1.0, 1.0, if is_dark { 0.14 } else { 0.20 }),
                    width: 1.0,
                    radius: 6.0.into(),
                },
                ..Default::default()
            }),
        ]
        .align_items(Alignment::Center)
        .padding([6, 0, 2, 0])
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
                let reachable = device.is_reachable();
                let kind = device.kind().label();
                let subtitle = if reachable {
                    format!("{} — Ready to receive", kind)
                } else {
                    format!("{} — Bluetooth only, not reachable yet", kind)
                };
                let icon = widgets::device_icon(&device);
                let msg = Message::ChooseRecipient(device.clone());
                col.push(widgets::device_list_row(
                    &device.display_title(),
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
        let is_dark = *theme == Theme::Dark;
        let border_subtle = if is_dark {
            iced::Color::from_rgba(1.0, 1.0, 1.0, 0.10)
        } else {
            iced::Color::from_rgba(0.0, 0.0, 0.0, 0.10)
        };

        let dialog = container(
            column![
                row![
                    text("Send Link")
                        .size(16)
                        .style(styles::text_color(*theme)),
                    Space::with_width(Length::Fill),
                    button(svg(icons::close()).width(16).height(16))
                        .on_press(Message::HideLinkDialog)
                        .style(iced::theme::Button::Text)
                        .padding([4, 4]),
                ]
                .align_items(Alignment::Center)
                .width(Length::Fill),
                Space::with_height(14),
                text_input("https://…", self.link_url)
                    .on_input(Message::LinkInputChanged)
                    .width(Length::Fill)
                    .padding(10),
                Space::with_height(16),
                row![
                    button(text("Cancel").size(12))
                        .on_press(Message::HideLinkDialog)
                        .style(iced::theme::Button::Text)
                        .padding([8, 16]),
                    Space::with_width(Length::Fill),
                    button(text("Send").size(12))
                        .on_press_maybe(if !self.link_url.trim().is_empty() {
                            self.selected_device
                                .map(|d| Message::SendLink(d.clone(), self.link_url.to_string()))
                        } else {
                            None
                        })
                        .style(iced::theme::Button::Primary)
                        .padding([8, 20]),
                ]
                .align_items(Alignment::Center)
                .width(Length::Fill),
            ]
            .spacing(4)
            .width(Length::Fill),
        )
        .padding(22)
        .width(Length::Fixed(380.0))
        .style(move |_: &IcedTheme| iced::widget::container::Appearance {
            background: Some(iced::Background::Color(surface)),
            border: iced::Border {
                radius: 14.0.into(),
                width: 1.0,
                color: border_subtle,
            },
            shadow: iced::Shadow {
                color: iced::Color::from_rgba(0.0, 0.0, 0.0, if is_dark { 0.55 } else { 0.18 }),
                offset: iced::Vector::new(0.0, 12.0),
                blur_radius: 32.0,
            },
            ..Default::default()
        });

        dismissible_scrim(dialog.into(), Some(Message::HideLinkDialog))
    }
}
