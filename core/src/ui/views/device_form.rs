//! Device detail modal — live discovery details plus drag-to-send.

use iced::{
    widget::{
        button, column, container, horizontal_rule, image, row, scrollable, svg, text, Space,
    },
    Alignment, Background, Border, Element, Length, Theme as IcedTheme,
};

use crate::network::DiscoveredDevice;
use crate::protocols::airdrop::AirDropStatus;
use crate::ui::{
    assets,
    components,
    device_icons,
    distance::{device_distance_label, rssi_to_feet},
    icons,
    messages::Message,
    styles,
    Theme,
};

pub fn overlay<'a>(
    device: &'a DiscoveredDevice,
    drop_hover: bool,
    file_transfer_progress: Option<f32>,
    airdrop_status: &AirDropStatus,
    theme: &Theme,
) -> Element<'a, Message> {
    let is_dark = *theme == Theme::Dark;
    let dot = device_icons::radar_dot_color(device);
    let ble_only = !device.is_reachable();
    let idle = matches!(
        airdrop_status,
        AirDropStatus::Idle | AirDropStatus::Connected
    );
    let connecting = matches!(airdrop_status, AirDropStatus::Connecting);
    let transferring = file_transfer_progress.is_some() || connecting;
    let can_dismiss = !transferring;
    let drop_ready = drop_hover && !ble_only && idle && !transferring;

    let dialog = device_card(
        device,
        theme,
        is_dark,
        dot,
        ble_only,
        idle,
        drop_ready,
        transferring,
        connecting,
        file_transfer_progress,
    );

    super::main_view::dismissible_scrim(
        dialog,
        can_dismiss.then_some(Message::DeviceDeselected),
    )
}

fn device_card<'a>(
    device: &'a DiscoveredDevice,
    theme: &Theme,
    is_dark: bool,
    dot: iced::Color,
    ble_only: bool,
    idle: bool,
    drop_ready: bool,
    transferring: bool,
    connecting: bool,
    file_transfer_progress: Option<f32>,
) -> Element<'a, Message> {
    let surface = styles::surface(*theme);
    let border_subtle = if is_dark {
        iced::Color::from_rgba(1.0, 1.0, 1.0, 0.10)
    } else {
        iced::Color::from_rgba(0.0, 0.0, 0.0, 0.10)
    };
    let footer_bg = if is_dark {
        iced::Color::from_rgba(0.0, 0.0, 0.0, 0.22)
    } else {
        iced::Color::from_rgba(0.0, 0.0, 0.0, 0.03)
    };

    container(
        column![
            header(device, theme, dot, transferring),
            horizontal_rule(1),
            container(
                scrollable(live_details(device, theme))
                    .height(Length::Fixed(240.0)),
            )
            .width(Length::Fill)
            .padding([12, 20, 8, 20]),
            horizontal_rule(1),
            container(
                column![drop_panel(
                    theme,
                    is_dark,
                    ble_only,
                    drop_ready,
                    transferring,
                    connecting,
                    file_transfer_progress,
                ),]
                .width(Length::Fill)
                .padding([16, 24, 12, 24]),
            )
            .width(Length::Fill),
            horizontal_rule(1),
            footer(is_dark, footer_bg, idle, ble_only, transferring),
        ]
        .width(Length::Fill),
    )
    .width(Length::Fixed(440.0))
    .style(move |_: &IcedTheme| container::Appearance {
        background: Some(Background::Color(surface)),
        border: Border {
            color: border_subtle,
            width: 1.0,
            radius: 16.0.into(),
        },
        shadow: iced::Shadow {
            color: iced::Color::from_rgba(0.0, 0.0, 0.0, if is_dark { 0.55 } else { 0.18 }),
            offset: iced::Vector::new(0.0, 12.0),
            blur_radius: 32.0,
        },
        ..Default::default()
    })
    .into()
}

fn header<'a>(
    device: &'a DiscoveredDevice,
    theme: &Theme,
    dot: iced::Color,
    transferring: bool,
) -> Element<'a, Message> {
    let category = device_icons::radar_dot_label(device_icons::radar_dot_category(device));

    row![
        container(
            container(Space::new(Length::Fixed(16.0), Length::Fixed(16.0)))
                .width(Length::Fixed(16.0))
                .height(Length::Fixed(16.0))
                .style(move |_: &IcedTheme| container::Appearance {
                    background: Some(Background::Color(dot)),
                    border: Border {
                        radius: 8.0.into(),
                        color: iced::Color::from_rgba(1.0, 1.0, 1.0, 0.45),
                        width: 1.0,
                    },
                    ..Default::default()
                }),
        )
        .width(Length::Fixed(52.0))
        .height(Length::Fixed(52.0))
        .center_x()
        .center_y(),
        column![
            text(device.display_title())
                .size(17)
                .style(styles::text_color(*theme)),
            text(format!("{} · {}", device.kind().label(), category))
                .size(12)
                .style(styles::text_color_muted(*theme)),
            status_badge(device, theme),
        ]
        .spacing(4)
        .width(Length::Fill),
        button(svg(icons::close()).width(18).height(18))
            .on_press_maybe((!transferring).then(|| Message::DeviceDeselected))
            .style(iced::theme::Button::Text)
            .padding([6, 6]),
    ]
    .align_items(Alignment::Center)
    .spacing(14)
    .width(Length::Fill)
    .padding([18, 20, 14, 20])
    .into()
}

fn live_details<'a>(device: &'a DiscoveredDevice, theme: &Theme) -> Element<'a, Message> {
    let distance = device_distance_label(device.rssi).unwrap_or_else(|| "—".to_string());
    let rssi = device
        .rssi
        .map(|r| format!("{r} dBm"))
        .unwrap_or_else(|| "—".to_string());
    let feet_raw = device.rssi.map(rssi_to_feet);
    let address = if device.address.is_unspecified() {
        "Bluetooth only".to_string()
    } else {
        format!("{}:{}", device.address, device.port)
    };
    let hardware = device
        .hardware_identifier()
        .unwrap_or_else(|| "—".to_string());
    let airdrop = if device.airdrop_active() {
        "Share sheet open".to_string()
    } else {
        "Not advertising".to_string()
    };

    let mut rows = column![
        row![
            container(Space::new(Length::Fixed(8.0), Length::Fixed(8.0)))
                .style(|_: &IcedTheme| container::Appearance {
                    background: Some(Background::Color(styles::colors::SUCCESS)),
                    border: Border {
                        radius: 4.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
            Space::with_width(8),
            text("Live — distance and signal update as you move")
                .size(11)
                .style(styles::colors::SUCCESS),
        ]
        .align_items(Alignment::Center),
        Space::with_height(10),
        detail_row("Display name", &device.display_title(), theme),
        detail_row("Discovered name", &device.name, theme),
        detail_row("Device type", device.kind().label(), theme),
        detail_row("Hardware ID", &hardware, theme),
        detail_row("Distance", &distance, theme),
        detail_row("Signal (RSSI)", &rssi, theme),
        detail_row("Estimated range", &format!("~{} ft", feet_raw.unwrap_or(0)), theme),
        detail_row("Status", device.status_label(), theme),
        detail_row("Network", &address, theme),
        detail_row("Service", device.service_label(), theme),
        detail_row("AirDrop", &airdrop, theme),
    ]
    .spacing(6)
    .width(Length::Fill);

    for (key, value) in interesting_txt(device) {
        rows = rows.push(detail_row(&key, &value, theme));
    }

    rows.into()
}

fn interesting_txt(device: &DiscoveredDevice) -> Vec<(String, String)> {
    let keys = [
        "model",
        "rpMd",
        "am",
        "accessory_label",
        "platform",
        "device_class",
    ];
    let mut out = Vec::new();
    for key in keys {
        if let Some(v) = device.txt_records.get(key) {
            if !v.is_empty() {
                out.push((key.to_string(), v.clone()));
            }
        }
    }
    out
}

fn detail_row<'a>(label: &str, value: &str, theme: &Theme) -> Element<'a, Message> {
    row![
        text(label.to_string())
            .size(11)
            .style(styles::text_color_muted(*theme))
            .width(Length::Fixed(118.0)),
        text(value.to_string())
            .size(11)
            .style(styles::text_color(*theme))
            .width(Length::Fill),
    ]
    .width(Length::Fill)
    .into()
}

fn status_badge(device: &DiscoveredDevice, theme: &Theme) -> Element<'static, Message> {
    let ready = device.is_reachable();
    let (label, fg, bg) = if ready {
        (
            "Ready to receive",
            styles::colors::SUCCESS,
            iced::Color::from_rgba(0.18, 0.80, 0.44, 0.14),
        )
    } else {
        (
            "Bluetooth only — Wi‑Fi required",
            styles::text_color_muted(*theme),
            iced::Color::from_rgba(0.55, 0.55, 0.60, 0.14),
        )
    };

    container(text(label).size(11).style(fg))
        .padding([3, 8])
        .style(move |_: &IcedTheme| container::Appearance {
            background: Some(Background::Color(bg)),
            border: Border {
                radius: 6.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

fn drop_panel<'a>(
    theme: &Theme,
    is_dark: bool,
    ble_only: bool,
    drop_ready: bool,
    transferring: bool,
    connecting: bool,
    progress: Option<f32>,
) -> Element<'a, Message> {
    let (title, hint) = drop_copy(ble_only, drop_ready, transferring, connecting);

    let body: Element<'a, Message> = if let Some(p) = progress {
        column![
            text("Sending…")
                .size(13)
                .style(styles::text_color(*theme)),
            Space::with_height(8),
            components::primary_progress_bar(p).width(Length::Fill),
            Space::with_height(6),
            text(format!("{:.0}% complete", p.clamp(0.0, 100.0)))
                .size(11)
                .style(styles::text_color_muted(*theme)),
        ]
        .align_items(Alignment::Center)
        .width(Length::Fill)
        .into()
    } else if connecting {
        column![
            image(assets::toolbar_logo())
                .height(Length::Fixed(48.0))
                .width(Length::Fixed(48.0)),
            Space::with_height(8),
            text("Connecting…")
                .size(13)
                .style(styles::text_color_secondary(*theme)),
        ]
        .align_items(Alignment::Center)
        .into()
    } else {
        column![
            image(assets::toolbar_logo())
                .height(Length::Fixed(56.0))
                .width(Length::Fixed(56.0)),
            Space::with_height(8),
            text(title)
                .size(13)
                .style(if drop_ready {
                    styles::colors::PRIMARY
                } else {
                    styles::text_color(*theme)
                }),
            text(hint)
                .size(11)
                .style(styles::text_color_muted(*theme))
                .horizontal_alignment(iced::alignment::Horizontal::Center),
        ]
        .align_items(Alignment::Center)
        .spacing(2)
        .into()
    };

    let zone_bg = if drop_ready {
        iced::Color::from_rgba(0.0, 0.48, 1.0, if is_dark { 0.16 } else { 0.10 })
    } else if ble_only {
        iced::Color::from_rgba(0.45, 0.45, 0.50, if is_dark { 0.10 } else { 0.06 })
    } else if transferring {
        iced::Color::from_rgba(0.0, 0.48, 1.0, if is_dark { 0.08 } else { 0.05 })
    } else {
        iced::Color::from_rgba(0.45, 0.45, 0.50, if is_dark { 0.08 } else { 0.04 })
    };

    let zone_border = if drop_ready {
        styles::colors::PRIMARY
    } else if ble_only {
        iced::Color::from_rgba(0.5, 0.5, 0.55, 0.25)
    } else {
        iced::Color::from_rgba(0.5, 0.5, 0.55, if is_dark { 0.28 } else { 0.18 })
    };

    container(body)
        .width(Length::Fill)
        .height(Length::Fixed(148.0))
        .center_x()
        .center_y()
        .padding([10, 14])
        .style(move |_: &IcedTheme| container::Appearance {
            background: Some(Background::Color(zone_bg)),
            border: Border {
                color: zone_border,
                width: if drop_ready { 2.0 } else { 1.0 },
                radius: 14.0.into(),
            },
            ..Default::default()
        })
        .into()
}

fn drop_copy(
    ble_only: bool,
    drop_ready: bool,
    transferring: bool,
    connecting: bool,
) -> (&'static str, &'static str) {
    if transferring && !connecting {
        return ("Sending…", "Please keep this window open");
    }
    if connecting {
        return ("Connecting…", "Establishing a secure transfer");
    }
    if ble_only {
        return (
            "Not available",
            "This device must be on the same Wi‑Fi network before files can be sent",
        );
    }
    if drop_ready {
        return ("Release to send", "Files will transfer to this device");
    }
    (
        "Drop files to send",
        "Drag files or folders anywhere over this window",
    )
}

fn footer<'a>(
    is_dark: bool,
    footer_bg: iced::Color,
    idle: bool,
    ble_only: bool,
    transferring: bool,
) -> Element<'a, Message> {
    let enabled = idle && !ble_only && !transferring;
    let icon_btn = button(
        container(svg(icons::link()).width(20).height(20))
            .padding(10)
            .center_x()
            .center_y()
            .style(move |_: &IcedTheme| container::Appearance {
                background: Some(Background::Color(if enabled {
                    if is_dark {
                        iced::Color::from_rgba(1.0, 1.0, 1.0, 0.06)
                    } else {
                        iced::Color::from_rgba(0.0, 0.0, 0.0, 0.04)
                    }
                } else {
                    iced::Color::TRANSPARENT
                })),
                border: Border {
                    color: if enabled {
                        iced::Color::from_rgba(0.5, 0.5, 0.55, if is_dark { 0.25 } else { 0.15 })
                    } else {
                        iced::Color::from_rgba(0.5, 0.5, 0.55, 0.10)
                    },
                    width: 1.0,
                    radius: 20.0.into(),
                },
                ..Default::default()
            }),
    )
    .on_press_maybe(enabled.then(|| Message::ShowLinkDialog))
    .style(iced::theme::Button::Text)
    .padding(0);

    container(
        row![
            Space::with_width(Length::Fill),
            icon_btn,
            Space::with_width(Length::Fill),
        ]
        .align_items(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([12, 20, 14, 20])
    .style(move |_: &IcedTheme| container::Appearance {
        background: Some(Background::Color(footer_bg)),
        border: Border {
            radius: [0.0, 0.0, 16.0, 16.0].into(),
            ..Default::default()
        },
        ..Default::default()
    })
    .into()
}
