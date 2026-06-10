//! Widget personalizzati per l'interfaccia utente AirDropd
//!
//! Questo modulo contiene widget personalizzati e riutilizzabili
//! per creare un'esperienza utente coerente e moderna.

use iced::{
    widget::{
        button, column, container, mouse_area, row, text, Space, progress_bar,
        horizontal_rule, vertical_rule,
    },
    Alignment, Element, Length, Background, Color, Border, Shadow, Pixels,
    Theme as IcedTheme,
};

use crate::ui::messages::Message;
use crate::ui::styles;

/// Widget per visualizzare lo stato di connessione
pub fn connection_status<'a>(
    is_connected: bool,
    device_name: Option<&str>,
    _theme: &IcedTheme,
) -> Element<'a, Message> {
    let (status_text, status_color) = if is_connected {
        ("Connected", styles::colors::SUCCESS)
    } else {
        ("Disconnected", styles::colors::ERROR)
    };

    let status_indicator = container(
        text("●")
            .size(12)
            .style(status_color)
    )
    .width(Length::Fixed(20.0))
    .center_x();

    let status_content = if let Some(name) = device_name {
        column![
            text(status_text)
                .size(12)
                .style(Color::BLACK),
            text(name)
                .size(10)
                .style(Color::from_rgb(0.5, 0.5, 0.5)),
        ]
        .spacing(2)
    } else {
        column![
            text(status_text)
                .size(12)
                .style(Color::BLACK),
        ]
    };

    row![
        status_indicator,
        status_content,
    ]
    .align_items(Alignment::Center)
    .spacing(styles::spacing::SMALL)
    .into()
}

/// Widget per visualizzare il progresso di trasferimento
pub fn transfer_progress<'a>(
    progress: f32,
    file_name: &str,
    transfer_speed: Option<&str>,
    _theme: &IcedTheme,
) -> Element<'a, Message> {
    let progress_bar = progress_bar(0.0..=100.0, progress)
        .style(move |theme: &IcedTheme| progress_bar::Appearance {
            background: Background::Color(if theme == &IcedTheme::Dark {
                Color::from_rgb(0.2, 0.2, 0.2)
            } else {
                Color::from_rgb(0.9, 0.9, 0.9)
            }),
            bar: Background::Color(Color::from_rgb(0.2, 0.6, 1.0)),
            border_radius: 4.0.into(),
        })
        .height(Length::Fixed(8.0));

    let progress_text = text(format!("{:.1}%", progress))
        .size(12)
        .style(Color::from_rgb(0.5, 0.5, 0.5));

    let file_info = row![
        text(file_name)
            .size(14)
            .style(Color::BLACK),
        Space::with_width(Length::Fill),
        progress_text,
    ]
    .align_items(Alignment::Center);

    let speed_info = if let Some(speed) = transfer_speed {
        Some(
            text(speed)
                .size(10)
                .style(styles::colors::TEXT_MUTED)
        )
    } else {
        None
    };

    let mut content = column![
        file_info,
        Space::with_height(styles::spacing::SMALL),
        progress_bar,
    ]
    .spacing(0);

    if let Some(speed) = speed_info {
        content = content.push(Space::with_height(Pixels(styles::spacing::SMALL.0 / 2.0)));
        content = content.push(speed);
    }

    container(content)
        .padding(styles::spacing::MEDIUM.0)
        .style(move |theme: &IcedTheme| container::Appearance {
            background: Some(Background::Color(if theme == &IcedTheme::Dark {
                Color::from_rgb(0.15, 0.15, 0.15)
            } else {
                Color::from_rgb(0.98, 0.98, 0.98)
            })),
            border: Border::with_radius(8.0),
            shadow: Shadow::default(),
            text_color: None,
        })
        .width(Length::Fill)
        .into()
}

/// Widget per visualizzare le statistiche di rete
pub fn network_stats<'a>(
    upload_speed: &str,
    download_speed: &str,
    connected_devices: usize,
    _theme: &IcedTheme,
) -> Element<'a, Message> {
    let stat_item = |label: &str, value: &str| -> Element<'a, Message> {
        column![
            text(value)
                .size(16)
                .style(Color::BLACK),
            text(label)
                .size(10)
                .style(Color::from_rgb(0.5, 0.5, 0.5)),
        ]
        .align_items(Alignment::Center)
        .spacing(2)
        .into()
    };

    let stats = row![
        stat_item("Upload", upload_speed),
        vertical_rule(1),
        stat_item("Download", download_speed),
        vertical_rule(1),
        stat_item("Devices", &connected_devices.to_string()),
    ]
    .align_items(Alignment::Center)
    .spacing(styles::spacing::MEDIUM);

    container(stats)
        .padding(styles::spacing::MEDIUM.0)
        .style(move |theme: &IcedTheme| container::Appearance {
            background: Some(Background::Color(if theme == &IcedTheme::Dark {
                Color::from_rgb(0.15, 0.15, 0.15)
            } else {
                Color::from_rgb(0.98, 0.98, 0.98)
            })),
            border: Border::with_radius(8.0),
            shadow: Shadow::default(),
            text_color: None,
        })
        .width(Length::Fill)
        .into()
}

/// Widget per visualizzare un badge di stato
pub fn status_badge<'a>(
    text_content: &str,
    badge_type: BadgeType,
    _theme: &IcedTheme,
) -> Element<'a, Message> {
    let (bg_color, text_color) = match badge_type {
        BadgeType::Success => (styles::colors::SUCCESS, Color::WHITE),
        BadgeType::Warning => (styles::colors::WARNING, Color::BLACK),
        BadgeType::Error => (styles::colors::ERROR, Color::WHITE),
        BadgeType::Info => (styles::colors::INFO, Color::WHITE),
        BadgeType::Neutral => (styles::colors::SURFACE, styles::colors::TEXT_PRIMARY),
    };

    container(
        text(text_content)
            .size(10)
            .style(text_color)
    )
    .padding([2, 6])
    .style(move |_: &IcedTheme| container::Appearance {
        background: Some(Background::Color(bg_color)),
        border: Border::with_radius(12.0),
        shadow: Shadow::default(),
        text_color: None,
    })
    .into()
}

/// Tipi di badge disponibili
#[derive(Debug, Clone, Copy)]
pub enum BadgeType {
    Success,
    Warning,
    Error,
    Info,
    Neutral,
}

/// Widget per visualizzare un separatore con testo
pub fn text_separator<'a>(
    text_content: &str,
    _theme: &IcedTheme,
) -> Element<'a, Message> {
    row![
        horizontal_rule(1),
        
        container(
            text(text_content)
                .size(12)
                .style(styles::colors::TEXT_MUTED)
        )
        .padding([0, styles::spacing::MEDIUM.0 as u16]),
        
        horizontal_rule(1),
    ]
    .align_items(Alignment::Center)
    .into()
}

/// Widget per visualizzare un tooltip informativo
pub fn info_tooltip<'a>(
    content: Element<'a, Message>,
    _tooltip_text: &str,
    _theme: &IcedTheme,
) -> Element<'a, Message> {
    // Per ora restituiamo solo il contenuto, in futuro si può implementare un vero tooltip
    content
}

/// Widget per creare un layout a griglia responsive
pub fn responsive_grid<'a>(
    items: Vec<Element<'a, Message>>,
    columns: usize,
) -> Element<'a, Message> {
    let mut rows = Vec::new();
    let mut current_row = Vec::new();
    let len = items.len();
    
    for (index, item) in items.into_iter().enumerate() {
        current_row.push(item);
        
        if current_row.len() == columns || index == len - 1 {
            let row_element = row(current_row)
                .spacing(styles::spacing::MEDIUM)
                .align_items(Alignment::Start);
            rows.push(row_element.into());
            current_row = Vec::new();
        }
    }
    
    column(rows)
        .spacing(styles::spacing::MEDIUM)
        .into()
}

/// Widget per creare un header di sezione
pub fn section_header<'a>(
    title: &str,
    subtitle: Option<&str>,
    action_button: Option<Element<'a, Message>>,
    _theme: &IcedTheme,
) -> Element<'a, Message> {
    let title_text = text(title)
        .size(18)
        .style(styles::colors::TEXT_PRIMARY);

    let mut header_content = column![title_text];

    if let Some(sub) = subtitle {
        let subtitle_text = text(sub)
            .size(12)
            .style(styles::colors::TEXT_MUTED);
        header_content = header_content.push(subtitle_text);
    }

    let mut header_row = row![header_content].align_items(Alignment::Center);

    if let Some(button) = action_button {
        header_row = header_row.push(Space::with_width(Length::Fill));
        header_row = header_row.push(button);
    }

    container(header_row)
        .padding([0, 0, styles::spacing::MEDIUM.0 as u16, 0])
        .width(Length::Fill)
        .into()
}

/// Full-width tappable device row (reliable click targets on Windows).
pub fn device_list_row<'a>(
    device_name: &str,
    device_icon: &str,
    subtitle: &str,
    is_selected: bool,
    theme: &IcedTheme,
    message: Message,
) -> Element<'a, Message> {
    let is_dark = theme == &IcedTheme::Dark;
    let row_bg = if is_selected {
        Color::from_rgba(0.0, 0.48, 1.0, if is_dark { 0.22 } else { 0.12 })
    } else if is_dark {
        styles::colors::SURFACE
    } else {
        Color::from_rgb(0.94, 0.94, 0.96)
    };
    let border_color = if is_selected {
        styles::colors::PRIMARY
    } else if is_dark {
        Color::from_rgba(1.0, 1.0, 1.0, 0.10)
    } else {
        Color::from_rgba(0.0, 0.0, 0.0, 0.08)
    };
    let name_color = if is_dark {
        styles::colors::TEXT_PRIMARY
    } else {
        styles::colors::TEXT_PRIMARY_LIGHT
    };
    let subtitle_color = if is_dark {
        styles::colors::TEXT_MUTED
    } else {
        styles::colors::TEXT_MUTED_LIGHT
    };

    let content = container(
        row![
            container(text(device_icon).size(24))
                .width(Length::Fixed(44.0))
                .height(Length::Fixed(44.0))
                .center_x()
                .center_y()
                .style(move |_: &IcedTheme| container::Appearance {
                    background: Some(Background::Color(if is_dark {
                        styles::colors::SURFACE_VARIANT
                    } else {
                        Color::from_rgb(0.88, 0.88, 0.90)
                    })),
                    border: Border {
                        radius: 22.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
            column![
                text(device_name)
                    .size(14)
                    .style(name_color),
                text(subtitle)
                    .size(11)
                    .style(subtitle_color),
            ]
            .spacing(2)
            .width(Length::Fill),
            text("›")
                .size(18)
                .style(subtitle_color),
        ]
        .align_items(Alignment::Center)
        .spacing(12)
        .width(Length::Fill),
    )
    .padding([10, 14])
    .width(Length::Fill)
    .style(move |_: &IcedTheme| container::Appearance {
        background: Some(Background::Color(row_bg)),
        border: Border {
            radius: 10.0.into(),
            width: if is_selected { 1.5 } else { 1.0 },
            color: border_color,
        },
        ..Default::default()
    });

    mouse_area(
        button(content)
            .on_press(message)
            .style(iced::theme::Button::Text)
            .width(Length::Fill)
            .padding(0),
    )
    .into()
}

/// Returns the appropriate device icon for a service type
pub fn device_icon(service_type: &crate::network::ServiceType) -> &'static str {
    match service_type {
        crate::network::ServiceType::AirDrop => "📱",
        crate::network::ServiceType::AirPlay => "📺",
        _ => "💻",
    }
}

/// Widget per creare un pannello collassabile
pub fn collapsible_panel<'a>(
    title: &str,
    is_expanded: bool,
    content: Element<'a, Message>,
    on_toggle: Message,
    _theme: &IcedTheme,
) -> Element<'a, Message> {
    let toggle_icon = if is_expanded { "▼" } else { "▶" };
    
    let header = button(
        row![
            text(toggle_icon)
                .size(12)
                .style(styles::colors::TEXT_MUTED),
            
            Space::with_width(styles::spacing::SMALL),
            
            text(title)
                .size(14)
                .style(styles::colors::TEXT_PRIMARY),
        ]
        .align_items(Alignment::Center)
    )
    .on_press(on_toggle)
    .width(Length::Fill);

    let mut panel = column![header];

    if is_expanded {
        panel = panel.push(Space::with_height(styles::spacing::SMALL));
        panel = panel.push(content);
    }

    container(panel)
        .style(move |theme: &IcedTheme| container::Appearance {
            background: Some(Background::Color(if theme == &IcedTheme::Dark {
                Color::from_rgb(0.15, 0.15, 0.15)
            } else {
                Color::from_rgb(0.98, 0.98, 0.98)
            })),
            border: Border::with_radius(8.0),
            shadow: Shadow::default(),
            text_color: None,
        })
        .padding(styles::spacing::MEDIUM.0)
        .width(Length::Fill)
        .into()
}