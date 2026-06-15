//! Registration section in Settings — product key entry and demo status.

use iced::{
    widget::{button, column, container, row, text, text_input, Space},
    Alignment, Element, Length,
};

use crate::licensing::{LicenseStatus, LicenseStore};
use crate::ui::{messages::Message, styles, Theme};

#[derive(Debug, Clone)]
pub struct RegistrationView {
    pub key_input: String,
}

impl RegistrationView {
    pub fn from_config(cfg: &crate::config::AppConfig) -> Self {
        Self {
            key_input: cfg
                .license
                .license_key
                .clone()
                .unwrap_or_default(),
        }
    }

    pub fn set_key_input(&mut self, value: String) {
        self.key_input = value;
    }

    pub fn view(&self, cfg: &crate::config::AppConfig, theme: &Theme) -> Element<Message> {
        let mut fields = cfg.license.clone();
        let store = LicenseStore::new(&mut fields);
        let status = store.status();

        let status_line = match status {
            LicenseStatus::Registered => {
                if let Some(key) = store.formatted_key() {
                    format!("Registered — {key}")
                } else {
                    "Registered".to_string()
                }
            }
            LicenseStatus::Demo => {
                let mut demo = cfg.license.clone();
                let mut demo_store = LicenseStore::new(&mut demo);
                format!(
                    "Demo — {} AirDrop sends and {} QR uploads left this week (max {} MB per file)",
                    demo_store.demo_sends_remaining(),
                    demo_store.demo_qr_remaining(),
                    crate::licensing::demo_max_file_bytes() / (1024 * 1024)
                )
            }
        };

        let donate = row![
            button(text("Donate $10+ via CashApp").size(13))
                .on_press(Message::OpenCashAppDonation)
                .style(iced::theme::Button::Secondary)
                .padding([6, 12]),
            Space::with_width(Length::Fixed(8.0)),
            text("One-time donation · same key on Windows and macOS · 2 devices")
                .size(12)
                .style(styles::text_color_muted(*theme)),
        ]
        .align_items(Alignment::Center);

        let key_row = row![
            text_input("XXXX-XXXX-XXXX", &self.key_input)
                .on_input(Message::LicenseKeyInputChanged)
                .padding(8)
                .width(Length::FillPortion(2)),
            Space::with_width(Length::Fixed(8.0)),
            button(text("Activate").size(13))
                .on_press(Message::ActivateLicense)
                .style(iced::theme::Button::Primary)
                .padding([8, 14]),
        ]
        .align_items(Alignment::Center)
        .width(Length::Fill);

        let mut body = column![
            text(status_line).size(13).style(styles::text_color(*theme)),
            Space::with_height(8),
            text(
                "Support AirDropd with a one-time $10+ CashApp donation to Rhythmic Records, \
                 then enter the product key you receive.",
            )
            .size(12)
            .style(styles::text_color_muted(*theme)),
            Space::with_height(8),
            donate,
            Space::with_height(12),
            key_row,
        ]
        .spacing(4);

        if status == LicenseStatus::Registered {
            body = body.push(Space::with_height(8)).push(
                button(text("Deactivate on this computer").size(13))
                    .on_press(Message::DeactivateLicense)
                    .style(iced::theme::Button::Secondary)
                    .padding([6, 12]),
            );
        }

        section("Registration", body.into())
    }
}

fn section<'a>(title: &'a str, body: Element<'a, Message>) -> Element<'a, Message> {
    container(column![
        text(title).size(18),
        Space::with_height(8),
        body,
    ])
    .padding(16)
    .width(Length::Fill)
    .into()
}
