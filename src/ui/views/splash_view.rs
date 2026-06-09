//! Startup splash screen with fade-in / fade-out animation.

use iced::{
    widget::{container, image},
    Element, Length, Theme as IcedTheme,
};

use crate::ui::assets::SplashFrames;
use crate::ui::messages::Message;
use crate::ui::styles;

pub fn render(frames: &SplashFrames, tick: usize) -> Element<'static, Message> {
    let handle = frames.handle_for_tick(tick);
    let bg = styles::colors::BACKGROUND;

    let splash = image(handle.clone())
        .width(Length::Fixed(320.0))
        .height(Length::Fixed(320.0))
        .content_fit(iced::ContentFit::Contain);

    container(
        container(splash)
            .center_x()
            .center_y()
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x()
    .center_y()
    .style(move |_: &IcedTheme| iced::widget::container::Appearance {
        background: Some(iced::Background::Color(bg)),
        ..Default::default()
    })
    .into()
}
