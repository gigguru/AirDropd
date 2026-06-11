//! Live Activity panel — a real-time feed of every protocol event (BLE,
//! mDNS, HTTPS server, transfers) so field problems can be diagnosed at a
//! glance: did the iPhone connect? Did TLS fail? Was the archive rejected?

use iced::{
    widget::{button, column, container, horizontal_rule, row, scrollable, text, Space},
    Alignment, Element, Length, Theme as IcedTheme,
};

use crate::activity;
use crate::ui::{messages::Message, styles, Theme};

/// Receiver status shown above the feed.
pub struct ReceiverStatus {
    pub broadcast_name: String,
    pub address: Option<String>,
    pub discoverable: bool,
}

pub fn render<'a>(status: ReceiverStatus, theme: &Theme) -> Element<'a, Message> {
    let header = row![
        button(text("← Back").size(14))
            .on_press(Message::ShowMainView)
            .style(iced::theme::Button::Secondary),
        Space::with_width(styles::spacing::MEDIUM),
        text("Live Activity").size(24),
        Space::with_width(Length::Fill),
        button(text("Clear").size(14))
            .on_press(Message::ClearActivityLog)
            .style(iced::theme::Button::Secondary),
    ]
    .align_items(Alignment::Center)
    .padding(styles::spacing::MEDIUM.0);

    let surface = styles::surface(*theme);

    let status_line = if status.discoverable {
        format!(
            "Receiving as \"{}\"{} — visible to Apple devices",
            status.broadcast_name,
            status
                .address
                .map(|a| format!(" on {}:8770", a))
                .unwrap_or_default()
        )
    } else {
        "Receiving is OFF — enable visibility to accept transfers".to_string()
    };

    let status_card = container(
        column![
            text(status_line).size(13).style(styles::text_color(*theme)),
            Space::with_height(6),
            text(
                "Test from an iPhone: open Photos → Share → AirDrop and pick this PC. \
                 Every step the phone takes shows up below in real time.",
            )
            .size(11)
            .style(styles::text_color_muted(*theme)),
        ]
        .padding(12),
    )
    .width(Length::Fill)
    .style(move |_: &IcedTheme| iced::widget::container::Appearance {
        background: Some(iced::Background::Color(surface)),
        border: iced::Border {
            radius: 10.0.into(),
            ..Default::default()
        },
        ..Default::default()
    });

    let events = activity::snapshot();
    let feed: Element<'a, Message> = if events.is_empty() {
        container(
            text("No activity yet — events appear here as devices interact with this PC.")
                .size(12)
                .style(styles::text_color_muted(*theme)),
        )
        .padding(16)
        .into()
    } else {
        events
            .into_iter()
            .fold(column![].spacing(4).width(Length::Fill), |col, event| {
                let is_error = event.category == activity::Category::Error;
                let color = if is_error {
                    styles::colors::ERROR
                } else {
                    styles::text_color_secondary(*theme)
                };
                col.push(
                    row![
                        text(event.at.format("%H:%M:%S").to_string())
                            .size(11)
                            .style(styles::text_color_muted(*theme))
                            .width(Length::Fixed(64.0)),
                        text(event.category.icon()).size(12),
                        Space::with_width(8),
                        text(event.message).size(12).style(color),
                    ]
                    .align_items(Alignment::Center),
                )
            })
            .into()
    };

    let content = column![
        status_card,
        Space::with_height(styles::spacing::MEDIUM),
        scrollable(feed).height(Length::Fill),
    ]
    .padding(styles::spacing::MEDIUM.0);

    container(column![header, horizontal_rule(1), content])
        .padding(styles::spacing::MEDIUM.0)
        .into()
}
