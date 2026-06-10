//! About view for the AirDropd application.
//!
//! This view shows application information, credits, licenses, and useful links.

use iced::{
    widget::{
        button, column, container, row, scrollable, text, Space,
        horizontal_rule,
    },
    Alignment, Element, Length,
};

use crate::ui::Theme;

use crate::ui::{
    messages::Message,
    styles,
};
 
/// About view state.
#[derive(Debug, Clone)]
pub struct AboutView {
    app_version: String,
    build_date: String,
    commit_hash: Option<String>,
} 
 
impl AboutView {
    /// Create a new About view instance.
    pub fn new(
        app_version: String, 
        build_date: String,
        commit_hash: Option<String>,
    ) -> Self {
        Self {
            app_version,
            build_date,
            commit_hash,
        }
    }

    /// Render the About view.
    pub fn view(&self, theme: &Theme) -> Element<Message> {
        let header = row![
            button(
                text("← Back")
                    .size(14)
            )
            .on_press(Message::ShowMainView)
            .style(iced::theme::Button::Secondary),
            
            Space::with_width(styles::spacing::MEDIUM),
            
            text("About")
                .size(24)
                .style(styles::colors::TEXT_PRIMARY),
        ]
        .align_items(Alignment::Center)
        .padding(styles::spacing::MEDIUM.0);

        let content = scrollable(
            column![
                // Logo and main title
                self.app_header(theme),
                
                Space::with_height(styles::spacing::LARGE),
                
                // Version information
                self.version_info(theme),
                
                Space::with_height(styles::spacing::LARGE),
                
                // Description
                self.description(theme),
                
                Space::with_height(styles::spacing::LARGE),
                
                // Features
                self.features(theme),
                
                Space::with_height(styles::spacing::LARGE),
                
                // Credits
                self.credits(theme),
                
                Space::with_height(styles::spacing::LARGE),
                
                // Licenses
                self.licenses(theme),
                
                Space::with_height(styles::spacing::LARGE),
                
                // Links
                self.links(theme),
                
                Space::with_height(styles::spacing::XLARGE),
            ]
            .spacing(0)
        )
        .height(Length::Fill);

        container(
            column![
                header,
                horizontal_rule(1),
                content,
            ]
        )
        .padding(styles::spacing::MEDIUM.0)
        .into()
    }



    /// Application header with logo.
    fn app_header(&self, _theme: &Theme) -> Element<Message> {
        container(
            column![
                // Logo
                text("📱")
                    .size(64)
                    .style(styles::colors::TEXT_PRIMARY),
                
                Space::with_height(styles::spacing::MEDIUM),
                
                // Application name
                text("AirDropd")
                    .size(32)
                    .style(styles::colors::TEXT_PRIMARY),
                
                // Subtitle
                text("AirDrop for Windows")
                    .size(16)
                    .style(styles::colors::TEXT_MUTED),
            ]
            .align_items(Alignment::Center)
            .spacing(styles::spacing::SMALL)
        )
        .center_x()
        .width(Length::Fill)
        .into()
    }

    /// Version information.
    fn version_info(&self, _theme: &Theme) -> Element<Message> {
        let version_items = column![
            row![
                text("Version:")
                    .size(14)
                    .style(styles::colors::TEXT_PRIMARY)
                    .width(Length::FillPortion(1)),
                
                text(self.app_version.clone())
                    .size(14)
                    .style(styles::colors::TEXT_MUTED)
                    .width(Length::FillPortion(2)),
            ]
            .align_items(Alignment::Center),
            
            row![
                text("Build:")
                    .size(14)
                    .style(styles::colors::TEXT_PRIMARY)
                    .width(Length::FillPortion(1)),
                
                text(self.build_date.clone())
                    .size(14)
                    .style(styles::colors::TEXT_MUTED)
                    .width(Length::FillPortion(2)),
            ]
            .align_items(Alignment::Center),
        ]
        .spacing(styles::spacing::SMALL);

        let version_with_commit = if let Some(commit) = &self.commit_hash {
            column![
                version_items,
                
                row![
                    text("Commit:")
                        .size(14)
                        .style(styles::colors::TEXT_PRIMARY)
                        .width(Length::FillPortion(1)),
                    
                    text(commit.clone())
                        .size(14)
                        .style(styles::colors::TEXT_MUTED)
                        .width(Length::FillPortion(2)),
                ]
                .align_items(Alignment::Center),
            ]
            .spacing(styles::spacing::SMALL)
        } else {
            version_items
        };

        container(
            column![
                text("Version")
                    .size(18)
                    .style(styles::colors::TEXT_SECONDARY),
                
                Space::with_height(styles::spacing::MEDIUM),
                
                version_with_commit,
            ]
        )
        .style(styles::container_secondary)
        .padding(styles::spacing::MEDIUM.0)
        .width(Length::Fill)
        .into()
    }

    /// Application description.
    fn description(&self, _theme: &Theme) -> Element<Message> {
        container(
            column![
                text("Description")
                    .size(18)
                    .style(styles::colors::TEXT_SECONDARY),
                
                Space::with_height(styles::spacing::MEDIUM),
                
                text("AirDropd brings Apple AirDrop to Windows, letting you share files, folders, and links between Apple devices and your PC.")
                    .size(14)
                    .style(styles::colors::TEXT_PRIMARY),
                
                Space::with_height(styles::spacing::MEDIUM),
                
                text("The application uses standard network protocols to ensure compatibility and security in wireless communications.")
                    .size(14)
                    .style(styles::colors::TEXT_PRIMARY),
            ]
        )
        .style(styles::container_secondary)
        .padding(styles::spacing::MEDIUM.0)
        .width(Length::Fill)
        .into()
    }

    /// Main features.
    fn features(&self, theme: &Theme) -> Element<Message> {
        let features_list = column![
            (&self).feature_item("📁", "File & Folder Sharing", "Send and receive files via AirDrop", theme),
            (&self).feature_item("🔗", "Link Sharing", "Share URLs and web links", theme),
            (&self).feature_item("📡", "Distance Radar", "Devices placed by Bluetooth signal strength", theme),
            (&self).feature_item("🔍", "Automatic Discovery", "Find compatible devices automatically", theme),
            (&self).feature_item("🔒", "Security", "Encrypted and secure communications", theme),
            (&self).feature_item("⚡", "Performance", "Streamed transfers with live progress", theme),
        ]
        .spacing(styles::spacing::MEDIUM);
  
        container(
            column![
                text("Features")
                    .size(18)
                    .style(styles::colors::TEXT_SECONDARY),
                
                Space::with_height(styles::spacing::MEDIUM),
                
                features_list,
            ]
        )
        .style(styles::container_secondary)
        .padding(styles::spacing::MEDIUM.0)
        .width(Length::Fill)
        .into()
    }

    /// Single feature item.
    fn feature_item(
        &self,
        icon: &str,
        title: &str,
        description: &str,
        _theme: &Theme,
    ) -> Element<Message> {
        row![
            text(icon)
                .size(20)
                .width(Length::Fixed(40.0)),
            
            column![
                text(title)
                    .size(14)
                    .style(styles::colors::TEXT_PRIMARY),
                
                text(description)
                    .size(12)
                    .style(styles::colors::TEXT_MUTED),
            ]
            .spacing(iced::Pixels(styles::spacing::SMALL.0 / 2.0)),
        ]
        .align_items(Alignment::Center)
        .spacing(styles::spacing::MEDIUM)
        .into()
    }

    /// Credits and acknowledgements.
    fn credits(&self, _theme: &Theme) -> Element<Message> {
        container(
            column![
                text("Credits")
                    .size(18)
                    .style(styles::colors::TEXT_SECONDARY),
                
                Space::with_height(styles::spacing::MEDIUM),
                
                text("Built with ❤️ using:")
                    .size(14)
                    .style(styles::colors::TEXT_PRIMARY),
                
                Space::with_height(styles::spacing::SMALL),
                
                column![
                    text("• Rust - Programming language")
                        .size(12)
                        .style(styles::colors::TEXT_MUTED),
                    
                    text("• Iced - GUI framework")
                        .size(12)
                        .style(styles::colors::TEXT_MUTED),
                    
                    text("• Tokio - Async runtime")
                        .size(12)
                        .style(styles::colors::TEXT_MUTED),
                    
                    text("• mDNS-SD - Network service discovery")
                        .size(12)
                        .style(styles::colors::TEXT_MUTED),
                ]
                .spacing(iced::Pixels(styles::spacing::SMALL.0 / 2.0)),
                
                Space::with_height(styles::spacing::MEDIUM),
                
                text("Special thanks to the open source community for contributions and support.")
                    .size(12)
                    .style(styles::colors::TEXT_MUTED),
            ]
        )
        .style(styles::container_secondary)
        .padding(styles::spacing::MEDIUM.0)
        .width(Length::Fill)
        .into()
    }

    /// License information.
    fn licenses(&self, _theme: &Theme) -> Element<Message> {
        container(
            column![
                text("License")
                    .size(18)
                    .style(styles::colors::TEXT_SECONDARY),
                
                Space::with_height(styles::spacing::MEDIUM),
                
                text("AirDropd is distributed under Rhythmic Records.")
                    .size(14)
                    .style(styles::colors::TEXT_PRIMARY),
                
                Space::with_height(styles::spacing::MEDIUM),
                
                button(
                    text("📄 View Licenses")
                        .size(14)
                )
                .on_press(Message::OpenLicenses)
                .style(iced::theme::Button::Secondary),
            ]
        )
        .style(styles::container_secondary)
        .padding(styles::spacing::MEDIUM.0)
        .width(Length::Fill)
        .into()
    }

    /// Useful links.
    fn links(&self, _theme: &Theme) -> Element<Message> {
        container(
            column![
                text("Links")
                    .size(18)
                    .style(styles::colors::TEXT_SECONDARY),
                
                Space::with_height(styles::spacing::MEDIUM),
                
                row![
                    button(
                        text("🌐 Website")
                            .size(14)
                    )
                    .on_press(Message::OpenWebsite)
                    .style(iced::theme::Button::Secondary),
                    
                    button(
                        text("📚 Documentation")
                            .size(14)
                    )
                    .on_press(Message::OpenDocumentation)
                    .style(iced::theme::Button::Secondary),
                ]
                .spacing(styles::spacing::MEDIUM),
                
                row![
                    button(
                        text("🐛 Report Bug")
                            .size(14)
                    )
                    .on_press(Message::OpenIssues)
                    .style(iced::theme::Button::Secondary),
                    
                    button(
                        text("💡 Request Feature")
                            .size(14)
                    )
                    .on_press(Message::OpenFeatureRequest)
                    .style(iced::theme::Button::Secondary),
                ]
                .spacing(styles::spacing::MEDIUM),
                
                Space::with_height(styles::spacing::MEDIUM),
                
                text("For support, visit our GitHub repository or contact the development team.")
                    .size(12)
                    .style(styles::colors::TEXT_MUTED),
            ]
        )
        .style(styles::container_secondary)
        .padding(styles::spacing::MEDIUM.0)
        .width(Length::Fill)
        .into()
    }
}