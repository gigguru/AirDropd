//! Interactive AirDrop radar — devices positioned by physical distance.
//!
//! Distance comes from BLE signal strength (RSSI) when available; devices
//! seen only over Wi-Fi sit on the middle ring. Nodes are clickable and act
//! as drop highlights during drag-and-drop.

use iced::widget::canvas::{self, stroke, Canvas, Frame, Geometry, Path, Program, Stroke};
use iced::{
    alignment, mouse, Color, Element, Length, Pixels, Point, Rectangle, Renderer,
    Theme as IcedTheme,
};

use crate::network::DiscoveredDevice;
use crate::ui::messages::Message;
use crate::ui::styles;

const NODE_RADIUS: f32 = 24.0;
const RING_FRACTIONS: [f32; 3] = [0.38, 0.66, 0.94];

pub struct Radar<'a> {
    devices: &'a [DiscoveredDevice],
    selected: Option<&'a DiscoveredDevice>,
    is_scanning: bool,
    tick: u32,
    is_dark: bool,
    drop_hover: bool,
}

pub fn radar<'a>(
    devices: &'a [DiscoveredDevice],
    selected: Option<&'a DiscoveredDevice>,
    is_scanning: bool,
    tick: u32,
    theme: &IcedTheme,
    drop_hover: bool,
) -> Element<'a, Message> {
    Canvas::new(Radar {
        devices,
        selected,
        is_scanning,
        tick,
        is_dark: theme == &IcedTheme::Dark,
        drop_hover,
    })
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

/// Ring index (0 = closest) for a device based on its BLE signal strength.
fn ring_for_device(device: &DiscoveredDevice) -> usize {
    match device.rssi {
        Some(rssi) if rssi >= -55 => 0,
        Some(rssi) if rssi >= -75 => 1,
        Some(_) => 2,
        // Wi-Fi only: reachable but distance unknown — middle ring.
        None => 1,
    }
}

/// Deterministic node positions: devices share rings, spread evenly,
/// stable across refreshes because they are sorted by name.
fn node_positions(devices: &[DiscoveredDevice], bounds: Rectangle) -> Vec<(Point, usize)> {
    let center = Point::new(bounds.width / 2.0, bounds.height / 2.0);
    let max_r = (bounds.width.min(bounds.height) / 2.0 - NODE_RADIUS - 18.0).max(40.0);

    let mut ring_members: [Vec<usize>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    let mut order: Vec<usize> = (0..devices.len()).collect();
    order.sort_by(|&a, &b| devices[a].name.to_lowercase().cmp(&devices[b].name.to_lowercase()));
    for idx in order {
        ring_members[ring_for_device(&devices[idx])].push(idx);
    }

    let mut out = vec![(center, 0); devices.len()];
    for (ring, members) in ring_members.iter().enumerate() {
        let radius = max_r * RING_FRACTIONS[ring];
        let count = members.len().max(1) as f32;
        for (slot, &device_idx) in members.iter().enumerate() {
            // Start at the top, stagger alternate rings so labels don't collide.
            let offset = if ring % 2 == 1 { 0.5 } else { 0.0 };
            let angle = -std::f32::consts::FRAC_PI_2
                + (slot as f32 + offset) * std::f32::consts::TAU / count;
            let pos = Point::new(
                center.x + radius * angle.cos(),
                center.y + radius * angle.sin(),
            );
            out[device_idx] = (pos, ring);
        }
    }
    out
}

fn hit_test(devices: &[DiscoveredDevice], bounds: Rectangle, cursor: Point) -> Option<usize> {
    let local = Point::new(cursor.x - bounds.x, cursor.y - bounds.y);
    node_positions(devices, bounds)
        .iter()
        .enumerate()
        .find(|(_, (pos, _))| {
            let dx = pos.x - local.x;
            let dy = pos.y - local.y;
            (dx * dx + dy * dy).sqrt() <= NODE_RADIUS + 6.0
        })
        .map(|(idx, _)| idx)
}

fn device_emoji(device: &DiscoveredDevice) -> &'static str {
    device.kind().emoji()
}

#[derive(Default)]
pub struct RadarState {
    hovered: Option<usize>,
}

impl<'a> Program<Message> for Radar<'a> {
    type State = RadarState;

    fn update(
        &self,
        state: &mut Self::State,
        event: canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> (canvas::event::Status, Option<Message>) {
        match event {
            canvas::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                state.hovered = cursor
                    .position()
                    .and_then(|p| hit_test(self.devices, bounds, p));
                (canvas::event::Status::Ignored, None)
            }
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(pos) = cursor.position() {
                    if bounds.contains(pos) {
                        return match hit_test(self.devices, bounds, pos) {
                            Some(idx) => (
                                canvas::event::Status::Captured,
                                Some(Message::DeviceSelected(self.devices[idx].clone())),
                            ),
                            None => (
                                canvas::event::Status::Captured,
                                Some(Message::DeviceDeselected),
                            ),
                        };
                    }
                }
                (canvas::event::Status::Ignored, None)
            }
            _ => (canvas::event::Status::Ignored, None),
        }
    }

    fn draw(
        &self,
        state: &Self::State,
        renderer: &Renderer,
        _theme: &IcedTheme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let center = Point::new(bounds.width / 2.0, bounds.height / 2.0);
        let max_r = (bounds.width.min(bounds.height) / 2.0 - NODE_RADIUS - 18.0).max(40.0);

        let base_alpha = if self.is_dark { 1.0 } else { 0.85 };
        let ring_color = |alpha: f32| {
            Color::from_rgba(0.0, 0.48, 1.0, alpha * base_alpha)
        };

        // Static distance rings.
        for (i, fraction) in RING_FRACTIONS.iter().enumerate() {
            let alpha = 0.28 - i as f32 * 0.07;
            frame.stroke(
                &Path::circle(center, max_r * fraction),
                Stroke::default()
                    .with_color(ring_color(alpha))
                    .with_width(1.2),
            );
        }

        // Expanding sonar pulse while scanning.
        if self.is_scanning {
            let phase = (self.tick % 46) as f32 / 46.0;
            let pulse_r = max_r * phase;
            let fade = (1.0 - phase).powf(1.5) * 0.5;
            frame.stroke(
                &Path::circle(center, pulse_r),
                Stroke::default()
                    .with_color(ring_color(fade))
                    .with_width(2.0),
            );
            let phase2 = ((self.tick + 23) % 46) as f32 / 46.0;
            frame.stroke(
                &Path::circle(center, max_r * phase2),
                Stroke::default()
                    .with_color(ring_color((1.0 - phase2).powf(1.5) * 0.35))
                    .with_width(1.5),
            );
        }

        // Drag-and-drop target glow across the whole radar.
        if self.drop_hover {
            frame.stroke(
                &Path::circle(center, max_r + NODE_RADIUS * 0.6),
                Stroke {
                    line_dash: stroke::LineDash {
                        segments: &[8.0, 6.0],
                        offset: (self.tick / 2) as usize,
                    },
                    ..Stroke::default()
                        .with_color(ring_color(0.85))
                        .with_width(2.5)
                },
            );
        }

        // Center node: this computer.
        frame.fill(
            &Path::circle(center, 17.0),
            Color::from_rgba(0.0, 0.48, 1.0, 0.85),
        );
        frame.fill_text(canvas::Text {
            content: "🖥".to_string(),
            position: center,
            color: Color::WHITE,
            size: Pixels(15.0),
            horizontal_alignment: alignment::Horizontal::Center,
            vertical_alignment: alignment::Vertical::Center,
            ..Default::default()
        });

        let label_color = if self.is_dark {
            styles::colors::TEXT_PRIMARY
        } else {
            styles::colors::TEXT_PRIMARY_LIGHT
        };
        let muted = if self.is_dark {
            styles::colors::TEXT_MUTED
        } else {
            styles::colors::TEXT_MUTED_LIGHT
        };

        // Device nodes positioned by distance ring.
        let positions = node_positions(self.devices, bounds);
        for (idx, device) in self.devices.iter().enumerate() {
            let (pos, _ring) = positions[idx];
            let is_selected = self
                .selected
                .map(|s| s.name == device.name && s.address == device.address)
                .unwrap_or(false);
            let is_hovered = state.hovered == Some(idx);
            let ble_only = device.port == 0 || device.address.is_unspecified();

            let fill = if is_selected {
                Color::from_rgba(0.0, 0.48, 1.0, 0.95)
            } else if is_hovered {
                Color::from_rgba(0.0, 0.48, 1.0, 0.55)
            } else if self.is_dark {
                Color::from_rgba(0.20, 0.20, 0.22, 0.95)
            } else {
                Color::from_rgba(0.88, 0.88, 0.92, 0.95)
            };

            // Halo for selected / hovered nodes.
            if is_selected || is_hovered {
                frame.fill(
                    &Path::circle(pos, NODE_RADIUS + 6.0),
                    Color::from_rgba(0.0, 0.48, 1.0, 0.18),
                );
            }

            frame.fill(&Path::circle(pos, NODE_RADIUS), fill);
            let border = if ble_only {
                // Dashed border: Bluetooth-only, not yet reachable over Wi-Fi.
                Stroke {
                    line_dash: stroke::LineDash {
                        segments: &[3.0, 3.0],
                        offset: 0,
                    },
                    ..Stroke::default()
                        .with_color(ring_color(0.7))
                        .with_width(1.5)
                }
            } else {
                Stroke::default()
                    .with_color(ring_color(if is_selected { 1.0 } else { 0.55 }))
                    .with_width(if is_selected { 2.5 } else { 1.5 })
            };
            frame.stroke(&Path::circle(pos, NODE_RADIUS), border);

            frame.fill_text(canvas::Text {
                content: device_emoji(device).to_string(),
                position: pos,
                color: Color::WHITE,
                size: Pixels(19.0),
                horizontal_alignment: alignment::Horizontal::Center,
                vertical_alignment: alignment::Vertical::Center,
                ..Default::default()
            });

            let mut label = device.name.clone();
            if label.chars().count() > 16 {
                label = format!("{}…", label.chars().take(15).collect::<String>());
            }
            frame.fill_text(canvas::Text {
                content: label,
                position: Point::new(pos.x, pos.y + NODE_RADIUS + 4.0),
                color: label_color,
                size: Pixels(11.0),
                horizontal_alignment: alignment::Horizontal::Center,
                vertical_alignment: alignment::Vertical::Top,
                ..Default::default()
            });
            // Device-type line, e.g. "iPhone" or "MacBook · Bluetooth".
            let airdrop_active = device.txt_records.contains_key("airdrop_active");
            let kind_line = if airdrop_active {
                "AirDrop open · Bluetooth".to_string()
            } else if ble_only {
                format!("{} · Bluetooth", device.kind().label())
            } else {
                device.kind().label().to_string()
            };
            frame.fill_text(canvas::Text {
                content: kind_line,
                position: Point::new(pos.x, pos.y + NODE_RADIUS + 17.0),
                color: muted,
                size: Pixels(9.0),
                horizontal_alignment: alignment::Horizontal::Center,
                vertical_alignment: alignment::Vertical::Top,
                ..Default::default()
            });
        }

        // Empty-state hint inside the radar.
        if self.devices.is_empty() {
            let hint = if self.is_scanning {
                "Scanning for nearby devices…"
            } else {
                "No devices nearby yet"
            };
            frame.fill_text(canvas::Text {
                content: hint.to_string(),
                position: Point::new(center.x, center.y + max_r * 0.55),
                color: muted,
                size: Pixels(12.0),
                horizontal_alignment: alignment::Horizontal::Center,
                vertical_alignment: alignment::Vertical::Center,
                ..Default::default()
            });
        }

        // Drop hint banner.
        if self.drop_hover {
            frame.fill_text(canvas::Text {
                content: "Drop files to send".to_string(),
                position: Point::new(center.x, 14.0),
                color: ring_color(1.0),
                size: Pixels(14.0),
                horizontal_alignment: alignment::Horizontal::Center,
                vertical_alignment: alignment::Vertical::Center,
                ..Default::default()
            });
        }

        vec![frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        state: &Self::State,
        _bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if state.hovered.is_some() {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        }
    }
}
