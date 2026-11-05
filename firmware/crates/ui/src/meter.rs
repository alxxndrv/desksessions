//! A frame-rate overlay: the current fps, the drawing time, and a chart of the last two seconds of
//! frames. Drawn over a finished frame, so it costs nothing when it isn't used.

use core::fmt::Write as _;

use crate::canvas::{Canvas, Datum, TextStyle};
use crate::fonts::Fonts;
use crate::theme::{self, Rgb, mix};

/// Frames kept, one pixel column each: two seconds at 60 fps.
pub const SAMPLES: usize = 120;

/// The chart's top is two frames at 60 Hz.
const FULL_SCALE_US: u32 = 33_333;
/// One refresh of the panel.
const REFRESH_US: u32 = 16_667;
/// The fps and drawing time average over this much of the latest history.
const WINDOW_US: u32 = 1_000_000;

const PAD: i32 = 4;
const WIDTH: i32 = SAMPLES as i32 + 2 * PAD;
const HEIGHT: i32 = 52;
const CHART_TOP: i32 = 20;
const CHART_HEIGHT: i32 = HEIGHT - CHART_TOP - PAD;

/// Frame timings and their overlay.
#[derive(Clone, Debug)]
pub struct FrameMeter {
    fonts: Fonts,
    /// Time from one frame's start to the next's, µs. A ring; `next` is the oldest.
    frame_us: [u32; SAMPLES],
    /// Time spent drawing each frame, µs.
    draw_us: [u32; SAMPLES],
    next: usize,
    len: usize,
}

impl Default for FrameMeter {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameMeter {
    #[must_use]
    pub fn new() -> Self {
        Self {
            fonts: Fonts::load(),
            frame_us: [0; SAMPLES],
            draw_us: [0; SAMPLES],
            next: 0,
            len: 0,
        }
    }

    /// Adds one frame: how long since the previous one started, and how long it took to draw.
    pub fn record(&mut self, frame_us: u32, draw_us: u32) {
        self.frame_us[self.next] = frame_us;
        self.draw_us[self.next] = draw_us;
        self.next = (self.next + 1) % SAMPLES;
        self.len = (self.len + 1).min(SAMPLES);
    }

    /// Samples from newest to oldest, as (frame, draw) µs.
    fn newest_first(&self) -> impl Iterator<Item = (u32, u32)> + '_ {
        (1..=self.len).map(move |back| {
            let i = (self.next + SAMPLES - back) % SAMPLES;
            (self.frame_us[i], self.draw_us[i])
        })
    }

    /// Frames per second and the mean drawing time in ms, over the last second.
    #[must_use]
    pub fn summary(&self) -> (f32, f32) {
        let (mut frames, mut frame_sum, mut draw_sum) = (0u32, 0u64, 0u64);
        for (frame, draw) in self.newest_first() {
            frames += 1;
            frame_sum += u64::from(frame);
            draw_sum += u64::from(draw);
            if frame_sum >= u64::from(WINDOW_US) {
                break;
            }
        }
        if frames == 0 || frame_sum == 0 {
            return (0.0, 0.0);
        }
        let fps = frames as f32 * 1e6 / frame_sum as f32;
        (fps, draw_sum as f32 / frames as f32 / 1000.0)
    }

    /// Draws the overlay in the top-right corner of `canvas`.
    pub fn draw<B: AsRef<[u16]> + AsMut<[u16]>>(&self, canvas: &mut Canvas<B>) {
        let x = canvas.width() - WIDTH - PAD;
        let y = PAD;
        canvas.fill_rect_alpha(x, y, WIDTH, HEIGHT, 210, 0x000000);

        // The numbers: fps left, drawing time right, in the drawing bars' colour.
        let (fps, draw_ms) = self.summary();
        let mut text = heapless::String::<16>::new();
        // Whole numbers and tenths by hand: float formatting would add 20 KB to the device image.
        let _ = write!(text, "{} fps", (fps + 0.5) as u32);
        let middle = y + PAD + 6;
        let style = TextStyle::new(Datum::MiddleLeft, theme::TEXT.hex());
        canvas.draw_string(&self.fonts.label, text.as_bytes(), x + PAD, middle, style);
        text.clear();
        let tenths = (draw_ms * 10.0 + 0.5) as u32;
        let _ = write!(text, "draw {}.{} ms", tenths / 10, tenths % 10);
        let style = TextStyle::new(Datum::MiddleRight, theme::JUMP.hex());
        canvas.draw_string(&self.fonts.small, text.as_bytes(), x + WIDTH - PAD, middle, style);

        // The chart: one column per frame, newest on the right. The frame's whole time, coloured by
        // how many refreshes it took, with the part spent drawing in front of it.
        let bottom = y + CHART_TOP + CHART_HEIGHT;
        let height = |us: u32| {
            let us = us.min(FULL_SCALE_US);
            ((us as u64 * CHART_HEIGHT as u64 + FULL_SCALE_US as u64 / 2) / FULL_SCALE_US as u64)
                as i32
        };
        let dim = |color: Rgb| mix(color, theme::BACKGROUND, 0.45).hex();
        let mut column = x + PAD + SAMPLES as i32 - 1;
        for (frame, draw) in self.newest_first() {
            let color = if frame <= REFRESH_US + REFRESH_US / 10 {
                theme::WORKING
            } else if frame <= 2 * REFRESH_US + REFRESH_US / 10 {
                theme::DONE
            } else {
                theme::NEEDS_YOU
            };
            let h = height(frame).max(1);
            canvas.draw_fast_vline(column, bottom - h, h, dim(color));
            let h = height(draw);
            if h > 0 {
                canvas.draw_fast_vline(column, bottom - h, h, theme::JUMP.hex());
            }
            column -= 1;
        }

        // A dotted line at one refresh: bars reaching above it missed the panel.
        let line = bottom - height(REFRESH_US);
        let faint = theme::TEXT_FAINT.hex();
        for dot in (x + PAD..x + PAD + SAMPLES as i32).step_by(3) {
            canvas.draw_pixel(dot, line, faint);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_covers_the_last_second() {
        let mut meter = FrameMeter::new();
        assert_eq!(meter.summary(), (0.0, 0.0));
        for _ in 0..30 {
            meter.record(33_333, 20_000); // slow, and older than a second by the end
        }
        for _ in 0..60 {
            meter.record(16_667, 8_000);
        }
        let (fps, draw_ms) = meter.summary();
        assert!((fps - 60.0).abs() < 0.1, "{fps}");
        assert!((draw_ms - 8.0).abs() < 0.01, "{draw_ms}");
    }

    #[test]
    fn the_ring_keeps_the_newest() {
        let mut meter = FrameMeter::new();
        for i in 0..SAMPLES as u32 + 5 {
            meter.record(i, 0);
        }
        let newest: heapless::Vec<u32, 3> = meter.newest_first().map(|(f, _)| f).take(3).collect();
        assert_eq!(newest.as_slice(), &[124, 123, 122]);
        assert_eq!(meter.newest_first().count(), SAMPLES);
    }
}
