//! Device-type icons and accent colors for the radar and device lists.

use iced::Color;

use crate::network::discovery::{is_airtag_beacon, is_android_device};
use crate::network::{DeviceKind, DiscoveredDevice};

/// Sonar blip category for color coding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadarDotCategory {
    Apple,
    Android,
    AirTag,
    Other,
}

pub fn radar_dot_category(device: &DiscoveredDevice) -> RadarDotCategory {
    radar_dot_category_for(device.kind(), device)
}

pub fn radar_dot_category_for(kind: DeviceKind, device: &DiscoveredDevice) -> RadarDotCategory {
    if is_airtag_beacon(kind) {
        RadarDotCategory::AirTag
    } else if is_android_device(kind)
        || (kind == DeviceKind::Unknown
            && device.txt_records.get("platform").map(|p| p.eq_ignore_ascii_case("android")) == Some(true))
    {
        RadarDotCategory::Android
    } else if matches!(
        kind,
        DeviceKind::IPhone
            | DeviceKind::IPad
            | DeviceKind::IPod
            | DeviceKind::MacBook
            | DeviceKind::IMac
            | DeviceKind::MacDesktop
            | DeviceKind::AppleWatch
            | DeviceKind::AppleTv
            | DeviceKind::AirPods
    ) || (kind == DeviceKind::Unknown
        && device.txt_records.get("apple_presence").map(|v| v == "1") == Some(true))
    {
        RadarDotCategory::Apple
    } else if kind == DeviceKind::Unknown
        && device.txt_records.get("mobile_presence").map(|v| v == "1") == Some(true)
    {
        RadarDotCategory::Other
    } else {
        RadarDotCategory::Other
    }
}

/// High-visibility sonar dot color by device family.
pub fn radar_dot_color(device: &DiscoveredDevice) -> Color {
    radar_dot_color_for(device.kind(), device)
}

pub fn radar_dot_color_for(kind: DeviceKind, device: &DiscoveredDevice) -> Color {
    match radar_dot_category_for(kind, device) {
        RadarDotCategory::Apple => Color::from_rgb(0.78, 0.80, 0.84),
        RadarDotCategory::Android => Color::from_rgb(0.22, 0.86, 0.46),
        RadarDotCategory::AirTag => Color::from_rgb(0.96, 0.82, 0.18),
        RadarDotCategory::Other => Color::from_rgb(0.0, 0.48, 1.0),
    }
}

pub fn radar_dot_label(category: RadarDotCategory) -> &'static str {
    match category {
        RadarDotCategory::Apple => "Apple",
        RadarDotCategory::Android => "Android",
        RadarDotCategory::AirTag => "AirTag / tracker",
        RadarDotCategory::Other => "Other",
    }
}

/// Icon glyph for a device category (emoji or symbol).
pub fn icon(device: &DiscoveredDevice) -> &'static str {
    device.kind().emoji()
}

/// Icon glyph for a device category.
pub fn icon_for(kind: DeviceKind) -> &'static str {
    kind.emoji()
}

/// Subtle ring / badge tint so device types are recognizable at a glance.
pub fn accent_color(kind: DeviceKind, is_dark: bool) -> Color {
    let alpha = if is_dark { 0.85 } else { 0.75 };
    match kind {
        DeviceKind::IPhone => Color::from_rgba(0.0, 0.48, 1.0, alpha),
        DeviceKind::IPad => Color::from_rgba(0.55, 0.35, 0.95, alpha),
        DeviceKind::IPod => Color::from_rgba(0.95, 0.35, 0.55, alpha),
        DeviceKind::MacBook => Color::from_rgba(0.45, 0.55, 0.65, alpha),
        DeviceKind::IMac => Color::from_rgba(0.35, 0.65, 0.85, alpha),
        DeviceKind::MacDesktop => Color::from_rgba(0.50, 0.50, 0.55, alpha),
        DeviceKind::AppleWatch => Color::from_rgba(0.95, 0.45, 0.35, alpha),
        DeviceKind::AppleTv => Color::from_rgba(0.25, 0.25, 0.30, alpha),
        DeviceKind::AirPods => Color::from_rgba(0.95, 0.95, 0.95, alpha),
        DeviceKind::AirTag => Color::from_rgba(0.95, 0.75, 0.20, alpha),
        DeviceKind::FindMyDevice => Color::from_rgba(0.95, 0.55, 0.15, alpha),
        DeviceKind::WindowsPc => Color::from_rgba(0.0, 0.45, 0.85, alpha),
        DeviceKind::AndroidPhone => Color::from_rgba(0.20, 0.75, 0.45, alpha),
        DeviceKind::AndroidTablet => Color::from_rgba(0.15, 0.65, 0.40, alpha),
        DeviceKind::AndroidWatch => Color::from_rgba(0.25, 0.70, 0.50, alpha),
        DeviceKind::AndroidTag => Color::from_rgba(0.30, 0.80, 0.55, alpha),
        DeviceKind::Unknown => Color::from_rgba(0.55, 0.55, 0.60, alpha),
    }
}
