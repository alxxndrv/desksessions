//! Backlight on a 12-bit LEDC channel. Levels are perceived lightness, turned into LED duty with
//! the CIE 1931 curve so fades look even all the way down; 4096 steps keep the low end smooth
//! where an 8-bit PWM visibly stairs.

use desk_core::math;
use esp_hal::ledc::LowSpeed;
use esp_hal::ledc::channel::{Channel, ChannelHW};

pub const BITS: u32 = 12;
pub const MAX_DUTY: u32 = (1 << BITS) - 1;
/// Lightness at level 0: dim, never fully dark.
const FLOOR: f32 = 0.04;

/// CIE 1931: relative luminance for lightness L* (both 0–1).
fn luminance(lightness: f32) -> f32 {
    if lightness <= 0.08 { lightness / 9.033 } else { math::powf((lightness + 0.16) / 1.16, 3.0) }
}

/// LED duty for a backlight `level` in [0, 1].
#[must_use]
pub fn duty_for(level: f32) -> u32 {
    let lightness = FLOOR + (1.0 - FLOOR) * level.clamp(0.0, 1.0);
    math::lround(luminance(lightness) * MAX_DUTY as f32) as u32
}

pub struct Backlight {
    channel: Channel<'static, LowSpeed>,
    last: Option<u32>,
}

impl Backlight {
    #[must_use]
    pub fn new(channel: Channel<'static, LowSpeed>) -> Self {
        Self { channel, last: None }
    }

    pub fn set(&mut self, level: f32) {
        let duty = duty_for(level);
        if self.last != Some(duty) {
            self.channel.set_duty_hw(duty);
            self.last = Some(duty);
        }
    }
}
