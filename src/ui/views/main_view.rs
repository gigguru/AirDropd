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
    airplay_status: &'a crate::protocols::airplay::AirPlayStatus,
    airdrop_status: &'a crate::protocols::airdrop::AirDropStatus,
    file_transfer_progress: Option<f32>,
    notifications: &'a [NotificationMessage],
    show_link_dialog: bool,
    link_url: &'a str,
    visibility: AirDropVisibility,
}

pub fn render<'a>(
    discovered_devices: &'a [crate::network::DiscoveredDevice],
    selected_device: Option<&'a crate::network::DiscoveredDevice>,
    is_scanning: bool,
    airplay_status: &'a crate::protocols::airplay::AirPlayStatus,
    airdrop_status: &'a crate::protocols::airdrop::AirDropStatus,
    file_transfer_progress: Option<f32>,
    notifications: &'a [NotificationMessage],
    show_link_dialog: bool,
    link_url: &'a str,
    visibility: AirDropVisibility,
    theme: &Theme,
) -> Element<'a, Message> {
    MainView::new(
        discovered_devices,
        selected_device,
        is_scanning,
        airplay_status,
        airdrop_status,
        file_transfer_progress,
        notifications,
        show_link_dialog,
        link_url,
        visibility,
    )
    .view(theme)
}

impl<'a> MainView<'a> {
    pub fn new(
        discovered_devices: &'a [crate::network::DiscoveredDevice],
        selected_device: Option<&'a crate::network::DiscoveredDevice>,
        is_scanning: bool,
        airplay_status: &'a crate::protocols::airplay::AirPlayStatus,
        airdrop_status: &'a crate::protocols::airdrop::AirDropStatus,
        file_transfer_progress: Option<f32>,
        notifications: &'a [NotificationMessage],
        show_link_dialog: bool,
        link_url: &'a str,
        visibility: AirDropVisibility,
    ) -> Self {
        Self {
            discovered_devices,
            selected_device,
            is_scanning,
            airplay_status,
            airdrop_status,
            file_transfer_progress,
            notifications,
            show_link_dialog,
            link_url,
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
            self.discovery_center(&iced_theme, theme),
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
            container(
                column![content, self.link_dialog(theme)]
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
        } else {
            content.into()
        }
    }

    /// Center discovery zone: radar rings with device bubbles directly beneath (macOS AirDrop layout).
    fn discovery_center(
        &self,
        iced_theme: &IcedTheme,
        theme: &Theme,
    ) -> Element<'a, Message> {
        let status_text = if self.is_scanning {
            "Looking for others..."
        } else if self.discovered_devices.is_empty() {
            "No devices found"
        } else {
            "Tap a device to share with"
        };

        container(
            column![
                widgets::airdrop_radar(iced_theme),
                Space::with_height(12),
                self.device_row(iced_theme),
                Space::with_height(8),
                text(status_text)
                    .size(13)
                    .style(styles::text_color_muted(*theme))
                    .horizontal_alignment(iced::alignment::Horizontal::Center),
            ]
            .align_items(Alignment::Center)
            .width(Length::Fill)
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x()
        .center_y()
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
            container(widgets::airdrop_radar(iced_theme))
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
                    let device_for_msg = device.clone();
                    row_el.push(widgets::device_bubble(
                        &device.name,
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
