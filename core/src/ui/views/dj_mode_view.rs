//! DJ Mode — QR on the left, file-cabinet drawers for each guest upload on the right.

use std::path::PathBuf;

use iced::widget::image::Handle;
use iced::{
    widget::{
        button, column, container, image, mouse_area, row, scrollable, text, text_input, Space,
    },
    Alignment, Element, Length, Theme as IcedTheme,
};

use crate::ui::{messages::Message, Theme};

const SILVER: iced::Color = iced::Color::from_rgb(0.75, 0.75, 0.78);
const CABINET: iced::Color = iced::Color::from_rgb(0.10, 0.10, 0.12);
const DRAWER_FACE: iced::Color = iced::Color::from_rgb(0.16, 0.16, 0.19);
const HANDLE: iced::Color = iced::Color::from_rgb(0.55, 0.55, 0.58);

#[derive(Debug, Clone)]
pub struct DjDrawer {
    pub folder_name: String,
    pub path: PathBuf,
    pub file_count: usize,
}

pub struct DjModeStatus<'a> {
    pub device_name: String,
    pub url: Option<String>,
    pub qr: Option<Handle>,
    pub server_listening: bool,
    pub files_received: u32,
    pub last_file: Option<String>,
    pub drawers: &'a [DjDrawer],
    pub renaming_folder: Option<&'a str>,
    pub rename_text: &'a str,
}

pub fn render<'a>(status: &DjModeStatus<'a>, _theme: &Theme) -> Element<'a, Message> {
    let bg = iced::Color::from_rgb(0.04, 0.04, 0.06);

    let exit = button(text("Exit DJ Mode").size(13))
        .on_press(Message::ExitDjMode)
        .style(iced::theme::Button::Secondary)
        .padding([8.0, 14.0]);

    let count_line = if status.files_received == 0 {
        text("Waiting for the first upload…")
            .size(14)
            .style(iced::Color::from_rgb(0.55, 0.55, 0.62))
    } else if status.files_received == 1 {
        text("1 file received")
            .size(14)
            .style(iced::Color::from_rgb(0.2, 0.85, 0.45))
    } else {
        text(format!("{} files received", status.files_received))
            .size(14)
            .style(iced::Color::from_rgb(0.2, 0.85, 0.45))
    };

    let last_line: Element<'a, Message> = match &status.last_file {
        Some(name) => text(format!("Latest: {}", name))
            .size(12)
            .style(iced::Color::from_rgb(0.55, 0.55, 0.62))
            .into(),
        None => Space::with_height(0).into(),
    };

    let header = column![
        row![
            column![
                text(format!("Send to {}", status.device_name))
                    .size(22)
                    .style(iced::Color::WHITE),
                Space::with_height(4),
                text("Open the Camera app → scan → pick files → tap Send")
                    .size(13)
                    .style(iced::Color::from_rgb(0.65, 0.65, 0.72)),
            ]
            .width(Length::Fill),
            exit,
        ]
        .align_items(Alignment::Center)
        .width(Length::Fill),
        Space::with_height(8),
        row![
            count_line,
            Space::with_width(16),
            container(
                text("Auto-save ON — each phone gets its own drawer folder")
                    .size(11)
                    .style(iced::Color::from_rgb(0.2, 0.85, 0.45)),
            )
            .padding([4, 10])
            .style(|_: &IcedTheme| iced::widget::container::Appearance {
                background: Some(iced::Background::Color(iced::Color::from_rgba(
                    0.2, 0.85, 0.45, 0.10,
                ))),
                border: iced::Border {
                    radius: 6.0.into(),
                    width: 1.0,
                    color: iced::Color::from_rgba(0.2, 0.85, 0.45, 0.30),
                },
                ..Default::default()
            }),
            Space::with_width(Length::Fill),
            last_line,
        ]
        .align_items(Alignment::Center)
        .width(Length::Fill),
    ]
    .padding([14.0, 20.0, 8.0, 20.0]);

    let body = row![
        qr_panel(status),
        cabinet_panel(status),
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .spacing(0);

    container(column![header, body].height(Length::Fill))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_: &IcedTheme| iced::widget::container::Appearance {
            background: Some(iced::Background::Color(bg)),
            ..Default::default()
        })
        .into()
}

fn qr_panel<'a>(status: &DjModeStatus<'a>) -> Element<'a, Message> {
    let qr_block: Element<'a, Message> = match (&status.qr, &status.url) {
        (Some(handle), Some(_)) => container(
            image(handle.clone())
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .padding(16)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x()
        .center_y()
        .style(|_: &IcedTheme| iced::widget::container::Appearance {
            background: Some(iced::Background::Color(iced::Color::WHITE)),
            border: iced::Border {
                radius: 16.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into(),
        _ => container(
            text("Connect this PC to Wi-Fi to show the scan code.")
                .size(14)
                .style(iced::Color::from_rgb(0.55, 0.55, 0.62)),
        )
        .padding(32)
        .center_x()
        .center_y()
        .width(Length::Fill)
        .height(Length::Fill)
        .into(),
    };

    container(
        column![
            text("Scan code")
                .size(13)
                .style(SILVER),
            Space::with_height(12),
            qr_block,
        ]
        .width(Length::Fill)
        .height(Length::Fill),
    )
    .width(Length::FillPortion(1))
    .height(Length::Fill)
    .padding([12, 16, 16, 20])
    .style(|_: &IcedTheme| iced::widget::container::Appearance {
        border: iced::Border {
            width: 0.0,
            color: iced::Color::TRANSPARENT,
            radius: 0.0.into(),
        },
        ..Default::default()
    })
    .into()
}

fn cabinet_panel<'a>(status: &DjModeStatus<'a>) -> Element<'a, Message> {
    let grid: Element<'a, Message> = if status.drawers.is_empty() {
        container(
            text("Guest sets appear here as silver drawers — right-click a drawer to rename")
                .size(13)
                .style(iced::Color::from_rgb(0.45, 0.45, 0.50))
                .horizontal_alignment(iced::alignment::Horizontal::Center),
        )
        .width(Length::Fill)
        .center_x()
        .padding(24)
        .into()
    } else {
        let mut rows: Vec<Element<'a, Message>> = Vec::new();
        for chunk in status.drawers.chunks(2) {
            let mut row_items = row![].spacing(10).width(Length::Fill);
            for drawer in chunk {
                row_items = row_items.push(drawer_widget(
                    drawer,
                    status.renaming_folder,
                    status.rename_text,
                ));
            }
            if chunk.len() == 1 {
                row_items = row_items.push(Space::with_width(Length::FillPortion(1)));
            }
            rows.push(row_items.into());
        }
        scrollable(column(rows).spacing(10).width(Length::Fill))
            .height(Length::Fill)
            .into()
    };

    container(
        column![
            row![
                text("Set cabinet")
                    .size(15)
                    .style(SILVER),
                Space::with_width(Length::Fill),
                text(format!("{} drawer{}", status.drawers.len(), if status.drawers.len() == 1 { "" } else { "s" }))
                    .size(11)
                    .style(iced::Color::from_rgb(0.45, 0.45, 0.50)),
            ]
            .align_items(Alignment::Center)
            .width(Length::Fill),
            Space::with_height(6),
            text("Click a drawer to open its folder · Right-click to rename · ↑↓ to reorder")
                .size(10)
                .style(iced::Color::from_rgb(0.40, 0.40, 0.44)),
            Space::with_height(10),
            grid,
        ]
        .width(Length::Fill)
        .height(Length::Fill),
    )
    .width(Length::FillPortion(1))
    .height(Length::Fill)
    .padding([12, 20, 16, 16])
    .style(|_: &IcedTheme| iced::widget::container::Appearance {
        background: Some(iced::Background::Color(CABINET)),
        border: iced::Border {
            color: iced::Color::from_rgba(1.0, 1.0, 1.0, 0.06),
            width: 1.0,
            radius: 12.0.into(),
        },
        ..Default::default()
    })
    .into()
}

fn drawer_widget<'a>(
    drawer: &DjDrawer,
    renaming: Option<&str>,
    rename_text: &str,
) -> Element<'a, Message> {
    let name = drawer.folder_name.clone();
    let is_renaming = renaming == Some(drawer.folder_name.as_str());

    let title: Element<'a, Message> = text(drawer.folder_name.clone())
        .size(13)
        .style(SILVER)
        .width(Length::Fill)
        .into();

    let subtitle = if drawer.file_count == 1 {
        "1 file".to_string()
    } else {
        format!("{} files", drawer.file_count)
    };

    if is_renaming {
        return container(
            column![
                text("Rename drawer")
                    .size(10)
                    .style(iced::Color::from_rgb(0.45, 0.45, 0.50)),
                Space::with_height(4),
                text_input("Drawer name…", rename_text)
                    .on_input(Message::DjDrawerRenameInput)
                    .on_submit(Message::DjDrawerRenameSubmit)
                    .padding(6)
                    .width(Length::Fill),
                text("Press Enter to save")
                    .size(9)
                    .style(iced::Color::from_rgb(0.40, 0.40, 0.44)),
            ]
            .width(Length::Fill)
            .padding([10, 12, 12, 12]),
        )
        .width(Length::FillPortion(1))
        .style(|_: &IcedTheme| iced::widget::container::Appearance {
            background: Some(iced::Background::Color(DRAWER_FACE)),
            border: iced::Border {
                color: iced::Color::from_rgba(0.0, 0.48, 1.0, 0.55),
                width: 1.5,
                radius: 6.0.into(),
            },
            ..Default::default()
        })
        .into();
    }

    let body = container(
        column![
            container(Space::new(Length::Fill, Length::Fixed(4.0)))
                .width(Length::Fixed(48.0))
                .height(Length::Fixed(4.0))
                .center_x()
                .style(|_: &IcedTheme| iced::widget::container::Appearance {
                    background: Some(iced::Background::Color(HANDLE)),
                    border: iced::Border {
                        radius: 2.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
            Space::with_height(8),
            title,
            Space::with_height(4),
            text(subtitle)
                .size(10)
                .style(iced::Color::from_rgb(0.45, 0.45, 0.50)),
        ]
        .width(Length::Fill)
        .padding([10, 12, 12, 12]),
    )
    .width(Length::Fill)
    .style(|_: &IcedTheme| iced::widget::container::Appearance {
        background: Some(iced::Background::Color(DRAWER_FACE)),
        border: iced::Border {
            color: iced::Color::from_rgba(0.75, 0.75, 0.78, 0.22),
            width: 1.0,
            radius: 6.0.into(),
        },
        ..Default::default()
    });

    let reorder = column![
        button(text("↑").size(11))
            .on_press(Message::DjDrawerMoveUp(name.clone()))
            .style(iced::theme::Button::Text)
            .padding([2, 6]),
        button(text("↓").size(11))
            .on_press(Message::DjDrawerMoveDown(name.clone()))
            .style(iced::theme::Button::Text)
            .padding([2, 6]),
    ]
    .spacing(2);

    let drawer_row = row![body.width(Length::FillPortion(1)), reorder]
        .align_items(Alignment::Center)
        .spacing(4);

    container(
        mouse_area(
            button(drawer_row)
                .on_press(Message::DjDrawerOpen(name.clone()))
                .style(iced::theme::Button::Text)
                .padding(0)
                .width(Length::Fill),
        )
        .on_right_press(Message::DjDrawerRenameStart(name)),
    )
    .width(Length::FillPortion(1))
    .into()
}
