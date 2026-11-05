use core::f32::consts::PI;

use crate::math;
use crate::time::{Millis, progress};

/// Apple's sleep-light rhythm (patent US6658577B2): a raised-cosine breath over 1.4 s followed
/// by 0.4 s of rest, repeating every 1.8 s.
pub const BREATH_PERIOD: Millis = Millis(1800);
pub const BREATH_REST: Millis = Millis(400);

/// Intensity in [0, 1] at `elapsed` since breathing began.
#[must_use]
pub fn breathe(elapsed: Millis) -> f32 {
    let active = BREATH_PERIOD - BREATH_REST;
    let period = BREATH_PERIOD.0;
    let phase = Millis(((elapsed.0 % period) + period) % period);
    if phase >= active {
        return 0.0;
    }
    let t = progress(phase, active);
    0.5 - 0.5 * math::cos(2.0 * PI * t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assert_float_eq;

    #[test]
    fn rises_peaks_and_rests() {
        assert_float_eq!(breathe(Millis(0)), 0.0);
        assert!((breathe(Millis(700)) - 1.0).abs() < 1e-5);
        assert_float_eq!(breathe(Millis(1500)), 0.0);
        assert!((breathe(Millis(1800 + 700)) - 1.0).abs() < 1e-5);
    }
}
