//! Embedded application icon and splash frame cache.

use iced::widget::image::Handle;
use iced::window;

pub const ICON_PNG: &[u8] = include_bytes!("../../../assets/airdropd-icon.png");

const TOOLBAR_LOGO_PX: u32 = 256;
const WINDOW_ICON_PX: u32 = 256;

static TOOLBAR_LOGO: std::sync::OnceLock<Handle> = std::sync::OnceLock::new();

fn rgba_from_embedded_png(max_px: u32) -> image::RgbaImage {
    let img = ::image::load_from_memory(ICON_PNG).expect("embedded app icon");
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    if w <= max_px && h <= max_px {
        return rgba;
    }
    ::image::imageops::resize(
        &rgba,
        max_px,
        max_px,
        ::image::imageops::FilterType::Lanczos3,
    )
}

/// Logo for the main toolbar (cached from the embedded app icon).
pub fn toolbar_logo() -> Handle {
    TOOLBAR_LOGO
        .get_or_init(|| {
            let rgba = rgba_from_embedded_png(TOOLBAR_LOGO_PX);
            let (width, height) = rgba.dimensions();
            Handle::from_pixels(width, height, rgba.into_raw())
        })
        .clone()
}

const SPLASH_STEPS: usize = 35;

/// Pre-rendered splash frames at increasing opacity (0 → 1).
pub struct SplashFrames {
    frames: Vec<Handle>,
    width: u32,
    height: u32,
}

impl SplashFrames {
    pub fn new() -> Self {
        let img = ::image::load_from_memory(ICON_PNG).expect("embedded splash icon");
        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();

        let mut frames = Vec::with_capacity(SPLASH_STEPS);
        for step in 0..SPLASH_STEPS {
            let opacity = step as f32 / (SPLASH_STEPS - 1) as f32;
            let mut pixels = rgba.clone().into_raw();
            for chunk in pixels.chunks_exact_mut(4) {
                chunk[3] = ((chunk[3] as f32) * opacity).round() as u8;
            }
            frames.push(Handle::from_pixels(width, height, pixels));
        }

        Self {
            frames,
            width,
            height,
        }
    }

    /// Map splash tick (0..=69) to a frame for fade-in then fade-out over 3.5 s.
    pub fn handle_for_tick(&self, tick: usize) -> &Handle {
        let idx = if tick < SPLASH_STEPS {
            tick
        } else {
            SPLASH_TOTAL_TICKS
                .saturating_sub(1)
                .saturating_sub(tick)
                .min(SPLASH_STEPS - 1)
        };
        &self.frames[idx]
    }

    #[allow(dead_code)]
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

pub const SPLASH_TICK_MS: u64 = 50;
pub const SPLASH_TOTAL_TICKS: usize = 70;

pub fn load_window_icon() -> Option<window::Icon> {
    let rgba = rgba_from_embedded_png(WINDOW_ICON_PX);
    let (width, height) = rgba.dimensions();
    window::icon::from_rgba(rgba.into_raw(), width, height).ok()
}
