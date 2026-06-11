//! DJ Mode — full-screen QR receive screen for live shows.
//!
//! Guests scan the code, pick files on their phone, and tracks land in the
//! download folder instantly with no prompts on this PC.

use iced::widget::image::Handle;
use iced::{
    widget::{button, column, container, image, row, text, Space},
    Alignment, Element, Length, Theme as IcedTheme,
};

use crate::ui::{messages::Message, Theme};

pub struct DjModeStatus {
    pub device_name: String,
    pub url: Option<String>,
    pub qr: Option<Handle>,
    pub files_received: u32,
    pub last_file: Option<String>,
}

pub fn render<'a>(status: &DjModeStatus, _theme: &Theme) -> Element<'a, Message> {
    let bg = iced::Color::from_rgb(0.04, 0.04, 0.06);

    let exit = button(text("Exit DJ Mode").size(13))
        .on_press(Message::ExitDjMode)
        .style(iced::theme::Button::Secondary)
        .padding([8.0, 14.0]);

    let top = row![
        Space::with_width(Length::Fill),
        exit,
    ]
    .align_items(Alignment::Center)
    .padding([16.0, 20.0]);

    let title = text(format!("Send to {}", status.device_name))
        .size(28)
        .style(iced::Color::WHITE);

    let qr_block: Element<'a, Message> = match (&status.qr, &status.url) {
        (Some(handle), Some(_)) => container(
            image(handle.clone())
                .width(Length::Fixed(420.0))
                .height(Length::Fixed(420.0)),
        )
        .padding(24)
        .style(|_: &IcedTheme| iced::widget::container::Appearance {
            background: Some(iced::Background::Color(iced::Color::WHITE)),
            border: iced::Border {
                radius: 24.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into(),
        _ => container(
            text("Connect this PC to Wi-Fi to show the scan code.")
                .size(16)
                .style(iced::Color::from_rgb(0.65, 0.65, 0.7)),
        )
        .padding(48)
        .into(),
    };

    let hint = text("Open the iPhone Camera → scan → pick files → tap Send")
        .size(15)
        .style(iced::Color::from_rgb(0.72, 0.72, 0.78));

    let auto_badge = container(
        text("Auto-save ON — each phone gets its own folder under WebDrop/")
            .size(12)
            .style(iced::Color::from_rgb(0.2, 0.85, 0.45)),
    )
    .padding([6.0, 12.0])
    .style(|_: &IcedTheme| iced::widget::container::Appearance {
        background: Some(iced::Background::Color(iced::Color::from_rgba(
            0.2, 0.85, 0.45, 0.12,
        ))),
        border: iced::Border {
            radius: 8.0.into(),
            width: 1.0,
            color: iced::Color::from_rgba(0.2, 0.85, 0.45, 0.35),
        },
        ..Default::default()
    });

    let count_line = if status.files_received == 0 {
        text("Waiting for the first upload…")
            .size(14)
            .style(iced::Color::from_rgb(0.55, 0.55, 0.62))
    } else if status.files_received == 1 {
        text("1 file received")
            .size(16)
            .style(iced::Color::from_rgb(0.2, 0.85, 0.45))
    } else {
        text(format!("{} files received", status.files_received))
            .size(16)
            .style(iced::Color::from_rgb(0.2, 0.85, 0.45))
    };

    let last_line: Element<'a, Message> = match &status.last_file {
        Some(name) => text(format!("Latest: {}", name))
            .size(13)
            .style(iced::Color::from_rgb(0.55, 0.55, 0.62))
            .into(),
        None => Space::with_height(18).into(),
    };

    let center = column![
        title,
        Space::with_height(20),
        qr_block,
        Space::with_height(18),
        hint,
        Space::with_height(12),
        auto_badge,
    ]
    .align_items(Alignment::Center)
    .spacing(4);

    let footer = column![count_line, last_line]
        .align_items(Alignment::Center)
        .spacing(6)
        .padding([0.0, 20.0, 28.0, 20.0]);

    container(
        column![
            top,
            container(center)
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x()
                .center_y(),
            footer,
        ]
        .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(move |_: &IcedTheme| iced::widget::container::Appearance {
        background: Some(iced::Background::Color(bg)),
        ..Default::default()
    })
    .into()
}
