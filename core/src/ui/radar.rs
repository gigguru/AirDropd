//! Interactive radar — devices placed by estimated distance with a rotating sweep.

use std::cell::RefCell;
use std::collections::HashMap;

use iced::widget::canvas::{self, stroke, Canvas, Frame, Geometry, Path, Program, Stroke};
use iced::{
    alignment, mouse, Color, Element, Length, Pixels, Point, Rectangle, Renderer,
    Theme as IcedTheme,
};

use crate::network::DiscoveredDevice;
use crate::ui::device_icons;
use crate::ui::distance;
use crate::ui::messages::Message;
use crate::ui::styles;

const NODE_RADIUS: f32 = 11.0;
const RING_FRACTIONS: [f32; 3] = distance::SONAR_RING_FRACTIONS;
const RING_FEET: [u32; 3] = [
    distance::SONAR_RING_FEET[0] as u32,
    distance::SONAR_RING_FEET[1] as u32,
    distance::SONAR_RING_FEET[2] as u32,
];
const SWEEP_TRAIL_STEPS: i32 = 48;
const POSITION_SMOOTH: f32 = 0.14;
/// Clockwise sweep speed (radians per animation tick) — PPI-style searchlight rotation.
const SWEEP_SPEED: f32 = 0.032;

fn color_with_alpha(color: Color, alpha: f32) -> Color {
    Color { a: alpha, ..color }
}

pub struct Radar<'a> {
    devices: &'a [DiscoveredDevice],
    selected: Option<&'a DiscoveredDevice>,
    sweep_active: bool,
    tick: u32,
    is_dark: bool,
    drop_hover: bool,
}

pub fn radar<'a>(
    devices: &'a [DiscoveredDevice],
    selected: Option<&'a DiscoveredDevice>,
    sweep_active: bool,
    tick: u32,
    theme: &IcedTheme,
    drop_hover: bool,
) -> Element<'a, Message> {
    Canvas::new(Radar {
        devices,
        selected,
        sweep_active,
        tick,
        is_dark: theme == &IcedTheme::Dark,
        drop_hover,
    })
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn device_key(device: &DiscoveredDevice) -> String {
    format!("{}|{}", device.name, device.address)
}

fn stable_angle(device: &DiscoveredDevice) -> f32 {
    let key = device_key(device);
    let mut hash: u32 = 0;
    for b in key.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(b as u32);
    }
    let t = hash as f32 / u32::MAX as f32;
    -std::f32::consts::FRAC_PI_2 + t * std::f32::consts::TAU
}

fn radius_fraction(device: &DiscoveredDevice) -> f32 {
    match device.rssi {
        Some(rssi) => distance::feet_to_radius_fraction(distance::rssi_to_feet(rssi) as f32),
        None => 0.58,
    }
}

fn radar_geometry(bounds: Rectangle) -> (Point, f32) {
    let center = Point::new(bounds.width / 2.0, bounds.height / 2.0);
    let max_r = (bounds.width.min(bounds.height) / 2.0 - NODE_RADIUS - 18.0).max(40.0);
    (center, max_r)
}

fn target_position(device: &DiscoveredDevice, bounds: Rectangle) -> Point {
    let (center, max_r) = radar_geometry(bounds);
    let angle = stable_angle(device);
    let r = max_r * radius_fraction(device);
    Point::new(
        center.x + r * angle.cos(),
        center.y + r * angle.sin(),
    )
}

fn smooth_positions(
    state: &RadarState,
    devices: &[DiscoveredDevice],
    bounds: Rectangle,
) -> Vec<Point> {
    let mut positions = state.positions.borrow_mut();
    let live: HashMap<String, ()> = devices.iter().map(|d| (device_key(d), ())).collect();
    positions.retain(|k, _| live.contains_key(k));

    devices
        .iter()
        .map(|device| {
            let key = device_key(device);
            let target = target_position(device, bounds);
            let pos = positions.entry(key).or_insert(target);
            pos.x += (target.x - pos.x) * POSITION_SMOOTH;
            pos.y += (target.y - pos.y) * POSITION_SMOOTH;
            *pos
        })
        .collect()
}

fn hit_test(
    state: &RadarState,
    devices: &[DiscoveredDevice],
    bounds: Rectangle,
    cursor: Point,
) -> Option<usize> {
    let local = Point::new(cursor.x - bounds.x, cursor.y - bounds.y);
    let positions = smooth_positions(state, devices, bounds);
    positions
        .iter()
        .enumerate()
        .find(|(_, pos)| {
            let dx = pos.x - local.x;
            let dy = pos.y - local.y;
            (dx * dx + dy * dy).sqrt() <= NODE_RADIUS + 10.0
        })
        .map(|(idx, _)| idx)
}

fn radar_dot_fill(
    base: iced::Color,
    is_selected: bool,
    is_hovered: bool,
    is_dark: bool,
) -> iced::Color {
    if is_selected {
        Color {
            a: 1.0,
            ..base
        }
    } else if is_hovered {
        color_with_alpha(base, 0.92)
    } else if is_dark {
        color_with_alpha(base, 0.88)
    } else {
        color_with_alpha(base, 0.82)
    }
}

pub struct RadarState {
    hovered: Option<usize>,
    positions: RefCell<HashMap<String, Point>>,
}

impl Default for RadarState {
    fn default() -> Self {
        Self {
            hovered: None,
            positions: RefCell::new(HashMap::new()),
        }
    }
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
                    .and_then(|p| hit_test(state, self.devices, bounds, p));
                (canvas::event::Status::Ignored, None)
            }
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(pos) = cursor.position() {
                    if bounds.contains(pos) {
                        return match hit_test(state, self.devices, bounds, pos) {
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
        let (center, max_r) = radar_geometry(bounds);

        let base_alpha = if self.is_dark { 1.0 } else { 0.85 };
        let ring_color = |alpha: f32| Color::from_rgba(0.0, 0.48, 1.0, alpha * base_alpha);

        for (i, fraction) in RING_FRACTIONS.iter().enumerate() {
            let alpha = 0.26 - i as f32 * 0.06;
            frame.stroke(
                &Path::circle(center, max_r * fraction),
                Stroke::default()
                    .with_color(ring_color(alpha))
                    .with_width(1.0),
            );
            frame.fill_text(canvas::Text {
                content: distance::format_feet(RING_FEET[i]),
                position: Point::new(center.x, center.y - max_r * fraction - 12.0),
                color: color_with_alpha(ring_color(1.0), 0.55),
                size: Pixels(10.0),
                horizontal_alignment: alignment::Horizontal::Center,
                vertical_alignment: alignment::Vertical::Bottom,
                ..Default::default()
            });
        }

        if self.sweep_active {
            // Clockwise searchlight sweep from north (12 o'clock).
            let sweep =
                -std::f32::consts::FRAC_PI_2 + (self.tick as f32 * SWEEP_SPEED) % std::f32::consts::TAU;

            for step in 0..SWEEP_TRAIL_STEPS {
                let angle = sweep - step as f32 * 0.022;
                let fade = (1.0 - step as f32 / SWEEP_TRAIL_STEPS as f32).powf(1.35) * 0.48;
                if fade < 0.015 {
                    continue;
                }
                let end = Point::new(
                    center.x + max_r * angle.cos(),
                    center.y + max_r * angle.sin(),
                );
                frame.stroke(
                    &Path::line(center, end),
                    Stroke::default()
                        .with_color(ring_color(fade))
                        .with_width(if step == 0 { 2.5 } else { 1.0 }),
                );
            }

            let wedge: Vec<Point> = (0..=18)
                .map(|i| {
                    let a = sweep - i as f32 * 0.010;
                    Point::new(
                        center.x + max_r * a.cos(),
                        center.y + max_r * a.sin(),
                    )
                })
                .chain(std::iter::once(center))
                .collect();
            frame.fill(
                &Path::new(|b| {
                    if let Some(first) = wedge.first() {
                        b.move_to(*first);
                        for pt in wedge.iter().skip(1) {
                            b.line_to(*pt);
                        }
                        b.close();
                    }
                }),
                color_with_alpha(ring_color(1.0), 0.08),
            );

            // Bright leading-edge glow on the active bearing line.
            let beam_end = Point::new(
                center.x + max_r * sweep.cos(),
                center.y + max_r * sweep.sin(),
            );
            frame.stroke(
                &Path::line(center, beam_end),
                Stroke::default()
                    .with_color(color_with_alpha(Color::WHITE, 0.55))
                    .with_width(1.5),
            );
        }

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

        frame.fill(
            &Path::circle(center, 18.0),
            Color::from_rgba(0.0, 0.48, 1.0, 0.92),
        );
        // Host device glyph — three horizontal bars (matches mockup center icon).
        for (i, width) in [(0, 14.0_f32), (1, 10.0), (2, 14.0)].iter() {
            let y = center.y - 4.0 + *i as f32 * 4.0;
            frame.stroke(
                &Path::line(
                    Point::new(center.x - width / 2.0, y),
                    Point::new(center.x + width / 2.0, y),
                ),
                Stroke::default()
                    .with_color(Color::WHITE)
                    .with_width(2.0),
            );
        }

        let muted = if self.is_dark {
            styles::colors::TEXT_MUTED
        } else {
            styles::colors::TEXT_MUTED_LIGHT
        };

        let positions = smooth_positions(state, self.devices, bounds);

        for (idx, device) in self.devices.iter().enumerate() {
            let pos = positions[idx];
            let is_selected = self
                .selected
                .map(|s| s.match_key() == device.match_key())
                .unwrap_or(false);
            let is_hovered = state.hovered == Some(idx);
            let ble_only = device.port == 0 || device.address.is_unspecified();

            let dot = device_icons::radar_dot_color(device);
            let fill = radar_dot_fill(dot, is_selected, is_hovered, self.is_dark);

            if is_selected || is_hovered {
                frame.fill(
                    &Path::circle(pos, NODE_RADIUS + 8.0),
                    color_with_alpha(dot, 0.28),
                );
            }

            frame.fill(&Path::circle(pos, NODE_RADIUS), fill);
            let border_color = if is_selected {
                Color::WHITE
            } else if ble_only {
                color_with_alpha(dot, 0.55)
            } else {
                color_with_alpha(Color::WHITE, 0.35)
            };
            let border = if ble_only {
                Stroke {
                    line_dash: stroke::LineDash {
                        segments: &[2.0, 2.0],
                        offset: 0,
                    },
                    ..Stroke::default()
                        .with_color(border_color)
                        .with_width(if is_selected { 2.0 } else { 1.0 })
                }
            } else {
                Stroke::default()
                    .with_color(border_color)
                    .with_width(if is_selected { 2.5 } else { 1.25 })
            };
            frame.stroke(&Path::circle(pos, NODE_RADIUS), border);
        }

        if self.devices.is_empty() {
            let hint = if !self.sweep_active {
                "Discovery frozen"
            } else {
                "No devices on sonar — switch to List for details"
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
