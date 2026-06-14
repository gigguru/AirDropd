//! Small toolbar SVG icons (settings gear, folder).

use iced::widget::svg;

use crate::ui::Theme;

fn toolbar_stroke(theme: &Theme) -> &'static str {
    if *theme == Theme::Dark {
        "#D8D8D8"
    } else {
        "#505050"
    }
}

fn toolbar_fill(theme: &Theme) -> &'static str {
    if *theme == Theme::Dark {
        "#D8D8D8"
    } else {
        "#505050"
    }
}

fn settings_svg(stroke: &str) -> String {
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="{stroke}" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">
  <path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"/>
  <circle cx="12" cy="12" r="3"/>
</svg>"##
    )
}

fn folder_svg(fill: &str) -> String {
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="{fill}">
  <path d="M10 4H4c-1.1 0-2 .9-2 2v12c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V8c0-1.1-.9-2-2-2h-8l-2-2z"/>
</svg>"##
    )
}

const LINK_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#888888" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"/>
  <path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"/>
</svg>"##;

const CLOSE_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#888888" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M18 6 6 18"/>
  <path d="m6 6 12 12"/>
</svg>"##;

const HEART_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#5A9CF5" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">
  <path d="M20.84 4.61a5.5 5.5 0 0 0-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 0 0-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 0 0 0-7.78z"/>
</svg>"##;

fn ice_cube_svg(stroke: &str) -> String {
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="{stroke}" stroke-width="1.75" stroke-linejoin="round">
  <path d="M12 2 7 5v4L2 12l5 3v4l5 3 5-3v-4l5-3-5-3V5z"/>
  <path d="M12 2v20"/>
  <path d="M7 5l10 7"/>
  <path d="M17 5 7 12"/>
  <path d="M7 19l10-7"/>
  <path d="M17 19 7 12"/>
</svg>"##
    )
}

fn handle(bytes: &[u8]) -> svg::Handle {
    svg::Handle::from_memory(bytes.to_vec())
}

pub fn settings(theme: &Theme) -> svg::Handle {
    handle(settings_svg(toolbar_stroke(theme)).as_bytes())
}

pub fn folder(theme: &Theme) -> svg::Handle {
    handle(folder_svg(toolbar_fill(theme)).as_bytes())
}

pub fn link() -> svg::Handle {
    handle(LINK_SVG.as_bytes())
}

pub fn close() -> svg::Handle {
    handle(CLOSE_SVG.as_bytes())
}

pub fn ice_cube(theme: &Theme) -> svg::Handle {
    handle(ice_cube_svg(toolbar_stroke(theme)).as_bytes())
}

pub fn heart() -> svg::Handle {
    handle(HEART_SVG.as_bytes())
}
