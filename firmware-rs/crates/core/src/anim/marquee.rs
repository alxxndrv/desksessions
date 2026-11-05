use crate::math;
use crate::time::{Millis, progress};

// Pebble's status-bar marquee timing.
pub const MARQUEE_LEAD_PAUSE: Millis = Millis(600);
pub const MARQUEE_TAIL_PAUSE: Millis = Millis(750);
pub const MARQUEE_PER_PIXEL: Millis = Millis(20);
pub const MARQUEE_REWIND_PER_PIXEL: Millis = Millis(2);

/// Leftward scroll offset (≥ 0) of text overflowing its box by `overflow` px, `elapsed` after it
/// was selected: pause, scroll to the end, pause, rewind fast, repeat.
#[must_use]
pub fn marquee_offset(overflow: f32, elapsed: Millis) -> f32 {
    if overflow <= 0.0 || elapsed.0 <= 0 {
        return 0.0;
    }
    let pixels = math::ceil(overflow) as i64;
    let scroll = MARQUEE_PER_PIXEL * pixels;
    let rewind = MARQUEE_REWIND_PER_PIXEL * pixels;
    let cycle = MARQUEE_LEAD_PAUSE + scroll + MARQUEE_TAIL_PAUSE + rewind;

    let mut t = elapsed.rem(cycle);
    if t < MARQUEE_LEAD_PAUSE {
        return 0.0;
    }
    t -= MARQUEE_LEAD_PAUSE;
    if t < scroll {
        return overflow * progress(t, scroll);
    }
    t -= scroll;
    if t < MARQUEE_TAIL_PAUSE {
        return overflow;
    }
    t -= MARQUEE_TAIL_PAUSE;
    overflow * (1.0 - progress(t, rewind))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assert_float_eq;

    #[test]
    fn pauses_scrolls_pauses_rewinds() {
        assert_float_eq!(marquee_offset(50.0, Millis(300)), 0.0);
        assert_float_eq!(marquee_offset(50.0, Millis(600 + 500)), 25.0);
        assert_float_eq!(marquee_offset(50.0, Millis(600 + 1000 + 300)), 50.0);
        assert_float_eq!(marquee_offset(50.0, Millis(600 + 1000 + 750 + 50)), 25.0);
        assert_float_eq!(marquee_offset(0.0, Millis(5000)), 0.0);
    }
}
