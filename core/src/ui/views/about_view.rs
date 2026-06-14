//! About dialog — branding, description, and optional CashApp support.

use iced::{
    widget::{button, column, container, image, mouse_area, row, svg, text, Space},
    Alignment, Background, Border, Element, Length, Theme as IcedTheme,
};

use crate::ui::{
    assets,
    icons,
    messages::Message,
    styles,
    views::main_view,
    Theme,
};

const CASHAPP_GREEN: iced::Color = iced::Color::from_rgb(0.0, 0.84, 0.39);
const TAGLINE_BLUE: iced::Color = iced::Color::from_rgb(0.35, 0.62, 0.98);

/// Modal About dialog with branded layout and support tile.
pub fn overlay(theme: &Theme) -> Element<'static, Message> {
    main_view::dismissible_scrim(about_card(theme), Some(Message::CloseAbout))
}

fn about_card(theme: &Theme) -> Element<'static, Message> {
    let is_dark = *theme == Theme::Dark;
    let surface = styles::surface(*theme);
    let border_subtle = if is_dark {
        iced::Color::from_rgba(1.0, 1.0, 1.0, 0.10)
    } else {
        iced::Color::from_rgba(0.0, 0.0, 0.0, 0.10)
    };
    let footer_bg = if is_dark {
        iced::Color::from_rgba(0.0, 0.0, 0.0, 0.22)
    } else {
        iced::Color::from_rgba(0.0, 0.0, 0.0, 0.04)
    };
    let tile_bg = if is_dark {
        iced::Color::from_rgb(0.20, 0.20, 0.22)
    } else {
        iced::Color::from_rgb(0.92, 0.92, 0.94)
    };

    container(
        column![
            container(
                row![
                    container(
                        image(assets::toolbar_logo())
                            .width(Length::Fixed(148.0))
                            .height(Length::Fixed(148.0))
                            .content_fit(iced::ContentFit::Contain),
                    )
                    .center_x()
                    .width(Length::Fixed(168.0)),
                    Space::with_width(Length::Fixed(20.0)),
                    column![
                        text("AirDropd")
                            .size(28)
                            .style(styles::colors::TEXT_PRIMARY),
                        Space::with_height(Length::Fixed(4.0)),
                        text("Seamless. Secure. Effortless.")
                            .size(15)
                            .style(TAGLINE_BLUE),
                        Space::with_height(Length::Fixed(14.0)),
                        text(
                            "AirDropd easily lets artists upload their audio file(s) using a QR \
                             code and sorts the files per device in each device's unique AirDropd \
                             folder.",
                        )
                        .size(13)
                        .style(styles::colors::TEXT_SECONDARY),
                        Space::with_height(Length::Fixed(20.0)),
                        row![
                            svg(icons::heart())
                                .width(Length::Fixed(22.0))
                                .height(Length::Fixed(22.0)),
                            Space::with_width(Length::Fixed(10.0)),
                            column![
                                text("Support further development of AirDropd")
                                    .size(14)
                                    .style(styles::colors::TEXT_PRIMARY),
                                Space::with_height(Length::Fixed(4.0)),
                                text(
                                    "Your support helps keep AirDropd improving for artists and \
                                     creators everywhere.",
                                )
                                .size(12)
                                .style(styles::colors::TEXT_MUTED),
                            ]
                            .width(Length::Fill),
                        ]
                        .align_items(Alignment::Start)
                        .width(Length::Fill),
                        Space::with_height(Length::Fixed(14.0)),
                        cashapp_tile(tile_bg),
                    ]
                    .width(Length::Fill)
                    .spacing(0),
                ]
                .align_items(Alignment::Start)
                .width(Length::Fill),
            )
            .padding([24, 28, 20, 28])
            .width(Length::Fill),
            container(
                row![
                    text("© 2026 Rhythmic Records. All rights reserved.")
                        .size(11)
                        .style(styles::colors::TEXT_MUTED),
                    Space::with_width(Length::Fill),
                    button(text("Close").size(13))
                        .on_press(Message::CloseAbout)
                        .padding([8, 20])
                        .style(iced::theme::Button::Secondary),
                ]
                .align_items(Alignment::Center)
                .width(Length::Fill),
            )
            .padding([12, 24, 14, 24])
            .width(Length::Fill)
            .style(move |_: &IcedTheme| container::Appearance {
                background: Some(Background::Color(footer_bg)),
                border: Border {
                    width: 0.0,
                    radius: styles::radius::LARGE.into(),
                    ..Default::default()
                },
                ..Default::default()
            }),
        ]
        .width(Length::Fill),
    )
    .width(Length::Fixed(620.0))
    .style(move |_: &IcedTheme| container::Appearance {
        background: Some(Background::Color(surface)),
        border: Border {
            color: border_subtle,
            width: 1.0,
            radius: styles::radius::LARGE.into(),
        },
        ..Default::default()
    })
    .into()
}

fn cashapp_tile(tile_bg: iced::Color) -> Element<'static, Message> {
    let icon = container(
        text("$")
            .size(22)
            .style(styles::colors::WHITE),
    )
    .width(Length::Fixed(44.0))
    .height(Length::Fixed(44.0))
    .center_x()
    .center_y()
    .style(move |_: &IcedTheme| container::Appearance {
        background: Some(Background::Color(CASHAPP_GREEN)),
        border: Border {
            radius: 10.0.into(),
            ..Default::default()
        },
        ..Default::default()
    });

    mouse_area(
        container(
            row![
                icon,
                Space::with_width(Length::Fixed(14.0)),
                column![
                    text("Donate via CashApp")
                        .size(13)
                        .style(styles::colors::TEXT_PRIMARY),
                    Space::with_height(Length::Fixed(2.0)),
                    text("$therealstollie")
                        .size(16)
                        .style(CASHAPP_GREEN),
                ]
                .align_items(Alignment::Start),
            ]
            .align_items(Alignment::Center)
            .width(Length::Fill),
        )
        .padding([12, 16])
        .width(Length::Fill)
        .style(move |_: &IcedTheme| container::Appearance {
            background: Some(Background::Color(tile_bg)),
            border: Border {
                color: iced::Color::from_rgba(1.0, 1.0, 1.0, 0.06),
                width: 1.0,
                radius: styles::radius::MEDIUM.into(),
            },
            ..Default::default()
        }),
    )
    .on_press(Message::OpenCashAppDonation)
    .into()
}
