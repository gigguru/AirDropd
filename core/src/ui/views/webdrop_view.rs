//! Web Drop screen — shows a QR code any phone can scan to send files to this
//! PC over the local network, with no app and no internet.

use iced::widget::image::Handle;
use iced::{
    widget::{column, container, horizontal_rule, image, row, text, Space},
    Alignment, Element, Length, Theme as IcedTheme,
};

use crate::ui::{messages::Message, styles, Theme};

pub struct WebDropStatus {
    pub url: Option<String>,
    pub qr: Option<Handle>,
}

pub fn render<'a>(status: &WebDropStatus, theme: &Theme) -> Element<'a, Message> {
    let header = row![
        iced::widget::button(text("← Back").size(14))
            .on_press(Message::ShowMainView)
            .style(iced::theme::Button::Secondary),
        Space::with_width(styles::spacing::MEDIUM),
        text("Receive via QR").size(24),
    ]
    .align_items(Alignment::Center)
    .padding(styles::spacing::MEDIUM.0);

    let surface = styles::surface(*theme);

    let qr_block: Element<'a, Message> = match (&status.qr, &status.url) {
        (Some(handle), Some(url)) => {
            let card = container(
                column![
                    image(handle.clone())
                        .width(Length::Fixed(300.0))
                        .height(Length::Fixed(300.0)),
                ]
                .align_items(Alignment::Center)
                .padding(20),
            )
            .style(|_: &IcedTheme| iced::widget::container::Appearance {
                background: Some(iced::Background::Color(iced::Color::WHITE)),
                border: iced::Border {
                    radius: 18.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            });

            column![
                card,
                Space::with_height(16),
                text(url.clone()).size(15).style(styles::text_color(*theme)),
            ]
            .align_items(Alignment::Center)
            .into()
        }
        _ => container(
            text("Connect this PC to Wi-Fi to show a scan code.")
                .size(15)
                .style(styles::text_color_muted(*theme)),
        )
        .padding(40)
        .into(),
    };

    let steps = container(
        column![
            step(theme, "1", "On the iPhone, open the Camera app"),
            step(theme, "2", "Point it at the code above and tap the yellow banner"),
            step(theme, "3", "Pick photos, videos or files and tap Send"),
            Space::with_height(8),
            text("The iPhone must be on the same Wi-Fi as this PC. Files arrive in your AirDropd download folder. Nothing leaves your network — no internet or app required.")
                .size(12)
                .style(styles::text_color_muted(*theme)),
        ]
        .spacing(10)
        .padding(18),
    )
    .width(Length::Fill)
    .style(move |_: &IcedTheme| iced::widget::container::Appearance {
        background: Some(iced::Background::Color(surface)),
        border: iced::Border {
            radius: 14.0.into(),
            ..Default::default()
        },
        ..Default::default()
    });

    let body = column![
        qr_block,
        Space::with_height(20),
        iced::widget::button(text("Start DJ Mode — full-screen QR").size(14))
            .on_press(Message::ShowDjMode)
            .style(iced::theme::Button::Primary)
            .padding([12, 20]),
        Space::with_height(24),
        steps,
    ]
        .align_items(Alignment::Center)
        .padding(styles::spacing::MEDIUM.0)
        .max_width(560);

    container(column![
        header,
        horizontal_rule(1),
        container(body).center_x().width(Length::Fill),
    ])
    .padding(styles::spacing::MEDIUM.0)
    .into()
}

fn step<'a>(theme: &Theme, num: &str, label: &str) -> Element<'a, Message> {
    row![
        container(text(num.to_string()).size(14).style(iced::Color::WHITE))
            .width(Length::Fixed(26.0))
            .height(Length::Fixed(26.0))
            .center_x()
            .center_y()
            .style(|_: &IcedTheme| iced::widget::container::Appearance {
                background: Some(iced::Background::Color(iced::Color::from_rgb(
                    0.04, 0.52, 1.0
                ))),
                border: iced::Border {
                    radius: 13.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }),
        Space::with_width(12),
        text(label.to_string())
            .size(14)
            .style(styles::text_color(*theme)),
    ]
    .align_items(Alignment::Center)
    .into()
}
