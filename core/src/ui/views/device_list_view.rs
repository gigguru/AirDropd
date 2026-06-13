//! Sortable list/grid of nearby devices (alternative to the sonar radar).

use iced::{
    widget::{button, column, container, row, scrollable, svg, text, Space, tooltip},
    Alignment, Element, Length, Theme as IcedTheme,
};

use crate::network::DiscoveredDevice;
use crate::ui::{
    device_icons,
    distance::{device_distance_label, rssi_to_feet},
    icons,
    messages::Message,
    styles,
    Theme,
};

/// Main-view device layout mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DeviceViewMode {
    #[default]
    Sonar,
    List,
}

/// Column used to sort the device list (click headers to change).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ListSortColumn {
    #[default]
    Distance,
    Name,
    Type,
    Model,
    Status,
}

impl ListSortColumn {
    pub fn label(self) -> &'static str {
        match self {
            Self::Name => "Device Name",
            Self::Type => "Device Type",
            Self::Model => "Hardware ID",
            Self::Distance => "Distance",
            Self::Status => "Status",
        }
    }
}

/// Return devices sorted by the active column and direction.
pub fn sort_devices(
    devices: &[DiscoveredDevice],
    column: ListSortColumn,
    ascending: bool,
) -> Vec<DiscoveredDevice> {
    let mut sorted: Vec<_> = devices.to_vec();
    sorted.sort_by(|a, b| {
        let ord = match column {
            ListSortColumn::Name => a
                .display_title()
                .to_ascii_lowercase()
                .cmp(&b.display_title().to_ascii_lowercase()),
            ListSortColumn::Type => a.kind().label().cmp(&b.kind().label()),
            ListSortColumn::Model => a
                .hardware_identifier()
                .unwrap_or_default()
                .to_ascii_lowercase()
                .cmp(
                    &b.hardware_identifier()
                        .unwrap_or_default()
                        .to_ascii_lowercase(),
                ),
            ListSortColumn::Distance => {
                let da = a.rssi.map(rssi_to_feet).unwrap_or(u32::MAX);
                let db = b.rssi.map(rssi_to_feet).unwrap_or(u32::MAX);
                da.cmp(&db)
            }
            ListSortColumn::Status => a.status_label().cmp(&b.status_label()),
        };
        if ascending {
            ord
        } else {
            ord.reverse()
        }
    });
    sorted
}

pub fn render<'a>(
    devices: &'a [DiscoveredDevice],
    selected: Option<&'a DiscoveredDevice>,
    sort_column: ListSortColumn,
    sort_ascending: bool,
    discovery_frozen: bool,
    theme: &Theme,
) -> Element<'a, Message> {
    let is_dark = *theme == Theme::Dark;
    let sorted = sort_devices(devices, sort_column, sort_ascending);
    let header = list_header(sort_column, sort_ascending, discovery_frozen, theme);

    if sorted.is_empty() {
        return container(
            column![
                header,
                Space::with_height(Length::FillPortion(1)),
                text(if discovery_frozen {
                    "Discovery frozen — no devices in snapshot"
                } else {
                    "No devices nearby"
                })
                .size(14)
                .style(styles::text_color_muted(*theme)),
                Space::with_height(Length::FillPortion(1)),
            ]
            .width(Length::Fill)
            .height(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .padding([4, 8])
        .into();
    }

    let rows: Element<'a, Message> = sorted
        .iter()
        .fold(column![].spacing(2).width(Length::Fill), |col, device| {
            let is_selected = selected
                .map(|s| s.name == device.name && s.address == device.address)
                .unwrap_or(false);
            col.push(list_row(device, is_selected, is_dark, theme))
        })
        .into();

    container(
        column![
            header,
            Space::with_height(4),
            scrollable(rows).height(Length::Fill),
        ]
        .width(Length::Fill)
        .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .padding([4, 8])
    .into()
}

fn list_header(
    active: ListSortColumn,
    ascending: bool,
    discovery_frozen: bool,
    theme: &Theme,
) -> Element<'static, Message> {
    let arrow = if ascending { " ▲" } else { " ▼" };
    let hdr = |col: ListSortColumn, width: Length| -> Element<'static, Message> {
        let label = if col == active {
            format!("{}{}", col.label(), arrow)
        } else {
            col.label().to_string()
        };
        let style = if col == active {
            iced::theme::Button::Primary
        } else {
            iced::theme::Button::Text
        };
        button(
            text(label)
                .size(10)
                .style(if col == active {
                    styles::colors::PRIMARY
                } else {
                    styles::text_color_secondary(*theme)
                }),
        )
        .on_press(Message::ListSortBy(col))
        .style(style)
        .padding([4, 4])
        .width(width)
        .into()
    };

    let freeze_btn = tooltip(
        button(
            svg(icons::ice_cube())
                .width(18)
                .height(18),
        )
        .on_press(Message::ToggleDiscoveryFreeze)
        .style(if discovery_frozen {
            iced::theme::Button::Primary
        } else {
            iced::theme::Button::Text
        })
        .padding([6, 8]),
        if discovery_frozen {
            "Discovery frozen — tap to resume"
        } else {
            "Freeze discovery"
        },
        tooltip::Position::Bottom,
    );

    row![
        Space::with_width(Length::Fixed(36.0)),
        hdr(ListSortColumn::Name, Length::FillPortion(3)),
        hdr(ListSortColumn::Type, Length::FillPortion(2)),
        hdr(ListSortColumn::Model, Length::FillPortion(2)),
        hdr(ListSortColumn::Distance, Length::FillPortion(2)),
        hdr(ListSortColumn::Status, Length::FillPortion(2)),
        freeze_btn,
    ]
    .align_items(Alignment::Center)
    .width(Length::Fill)
    .into()
}

fn list_row<'a>(
    device: &DiscoveredDevice,
    is_selected: bool,
    is_dark: bool,
    theme: &Theme,
) -> Element<'a, Message> {
    let kind = device.kind();
    let icon = device_icons::icon(device);
    let distance = device_distance_label(device.rssi).unwrap_or_else(|| "—".to_string());
    let hardware = device
        .hardware_identifier()
        .unwrap_or_else(|| "—".to_string());

    let row_bg = if is_selected {
        iced::Color::from_rgba(0.0, 0.48, 1.0, if is_dark { 0.22 } else { 0.12 })
    } else if is_dark {
        styles::colors::SURFACE
    } else {
        iced::Color::from_rgb(0.96, 0.96, 0.98)
    };
    let border = if is_selected {
        styles::colors::PRIMARY
    } else if is_dark {
        iced::Color::from_rgba(1.0, 1.0, 1.0, 0.06)
    } else {
        iced::Color::from_rgba(0.0, 0.0, 0.0, 0.06)
    };

    let content = row![
        container(text(icon).size(18))
            .width(Length::Fixed(36.0))
            .center_x()
            .center_y(),
        text(device.display_title())
            .size(12)
            .style(styles::text_color(*theme))
            .width(Length::FillPortion(3)),
        text(kind.label())
            .size(11)
            .style(styles::text_color_muted(*theme))
            .width(Length::FillPortion(2)),
        text(hardware)
            .size(11)
            .style(styles::text_color_muted(*theme))
            .width(Length::FillPortion(2)),
        text(distance)
            .size(11)
            .style(styles::text_color_muted(*theme))
            .width(Length::FillPortion(2)),
        text(device.status_label())
            .size(11)
            .style(if device.is_reachable() {
                styles::colors::SUCCESS
            } else {
                styles::text_color_muted(*theme)
            })
            .width(Length::FillPortion(2)),
    ]
    .align_items(Alignment::Center)
    .width(Length::Fill)
    .padding([6, 4]);

    button(
        container(content)
            .width(Length::Fill)
            .style(move |_: &IcedTheme| container::Appearance {
                background: Some(iced::Background::Color(row_bg)),
                border: iced::Border {
                    color: border,
                    width: if is_selected { 1.5 } else { 1.0 },
                    radius: 8.0.into(),
                },
                ..Default::default()
            }),
    )
    .on_press(Message::DeviceSelected(device.clone()))
    .style(iced::theme::Button::Text)
    .width(Length::Fill)
    .into()
}

/// Toolbar freeze button (shown in Sonar mode too).
pub fn freeze_button(discovery_frozen: bool) -> Element<'static, Message> {
    tooltip(
        button(
            svg(icons::ice_cube())
                .width(18)
                .height(18),
        )
        .on_press(Message::ToggleDiscoveryFreeze)
        .style(if discovery_frozen {
            iced::theme::Button::Primary
        } else {
            iced::theme::Button::Text
        })
        .padding([6, 8]),
        if discovery_frozen {
            "Discovery frozen — tap to resume"
        } else {
            "Freeze discovery"
        },
        tooltip::Position::Bottom,
    )
    .into()
}
