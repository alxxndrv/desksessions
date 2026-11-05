/// Cubic Bézier timing curve through (0,0), (x1,y1), (x2,y2), (1,1), as in
/// `CAMediaTimingFunction`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CubicBezier {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

// Core Animation's named timing functions.
pub const CA_DEFAULT: CubicBezier = CubicBezier::new(0.25, 0.1, 0.25, 1.0);
pub const CA_EASE_IN: CubicBezier = CubicBezier::new(0.42, 0.0, 1.0, 1.0);
pub const CA_EASE_OUT: CubicBezier = CubicBezier::new(0.0, 0.0, 0.58, 1.0);
pub const CA_EASE_IN_OUT: CubicBezier = CubicBezier::new(0.42, 0.0, 0.58, 1.0);

/// Polynomial coefficients of one Bézier axis: B(s) = ((a·s + b)·s + c)·s.
struct Axis {
    a: f32,
    b: f32,
    c: f32,
}

impl Axis {
    fn new(p1: f32, p2: f32) -> Self {
        Self { a: 1.0 - 3.0 * p2 + 3.0 * p1, b: 3.0 * p2 - 6.0 * p1, c: 3.0 * p1 }
    }

    fn at(&self, s: f32) -> f32 {
        ((self.a * s + self.b) * s + self.c) * s
    }

    fn slope(&self, s: f32) -> f32 {
        (3.0 * self.a * s + 2.0 * self.b) * s + self.c
    }

    /// Finds s with x(s) = x: Newton–Raphson first, bisection when the slope is too flat.
    fn solve(&self, x: f32) -> f32 {
        const EPSILON: f32 = 1e-5;
        let mut s = x;
        for _ in 0..8 {
            let error = self.at(s) - x;
            if error.abs() < EPSILON {
                return s;
            }
            let slope = self.slope(s);
            if slope.abs() < 1e-6 {
                break;
            }
            s -= error / slope;
        }

        let mut low = 0.0_f32;
        let mut high = 1.0_f32;
        s = x;
        let mut i = 0;
        while i < 32 && low < high {
            let value = self.at(s);
            if (value - x).abs() < EPSILON {
                break;
            }
            if x > value {
                low = s;
            } else {
                high = s;
            }
            s = (low + high) * 0.5;
            i += 1;
        }
        s
    }
}

impl CubicBezier {
    #[must_use]
    pub const fn new(x1: f32, y1: f32, x2: f32, y2: f32) -> Self {
        Self { x1, y1, x2, y2 }
    }

    /// Eased progress for linear progress `t` in [0, 1].
    #[must_use]
    pub fn at(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        if t == 0.0 || t == 1.0 {
            return t;
        }
        let x = Axis::new(self.x1, self.x2);
        let y = Axis::new(self.y1, self.y2);
        y.at(x.solve(t))
    }
}

/// Named curves used by tweens. The quadratic ones match Pebble's `animation_timing` tables.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Curve {
    Linear,
    /// Pebble: t²
    EaseIn,
    /// Pebble: 1 − (1 − t)²
    EaseOut,
    /// Pebble: both halves joined
    EaseInOut,
    CaDefault,
    CaEaseIn,
    CaEaseOut,
    CaEaseInOut,
}

/// Eased progress for linear progress `t`; `t` is clamped to [0, 1].
#[must_use]
pub fn ease(curve: Curve, t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    match curve {
        Curve::Linear => t,
        Curve::EaseIn => t * t,
        Curve::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
        Curve::EaseInOut => {
            if t < 0.5 {
                2.0 * t * t
            } else {
                1.0 - 2.0 * (1.0 - t) * (1.0 - t)
            }
        }
        Curve::CaDefault => CA_DEFAULT.at(t),
        Curve::CaEaseIn => CA_EASE_IN.at(t),
        Curve::CaEaseOut => CA_EASE_OUT.at(t),
        Curve::CaEaseInOut => CA_EASE_IN_OUT.at(t),
    }
}

#[must_use]
pub fn lerp(from: f32, to: f32, t: f32) -> f32 {
    from + (to - from) * t
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assert_float_eq;

    const ALL: [Curve; 8] = [
        Curve::Linear,
        Curve::EaseIn,
        Curve::EaseOut,
        Curve::EaseInOut,
        Curve::CaDefault,
        Curve::CaEaseIn,
        Curve::CaEaseOut,
        Curve::CaEaseInOut,
    ];

    #[test]
    fn endpoints_are_exact_for_every_curve() {
        for curve in ALL {
            assert_float_eq!(ease(curve, 0.0), 0.0);
            assert_float_eq!(ease(curve, 1.0), 1.0);
            assert_float_eq!(ease(curve, -1.0), 0.0);
            assert_float_eq!(ease(curve, 2.0), 1.0);
        }
    }

    #[test]
    fn pebble_quadratics_match_tables() {
        assert_float_eq!(ease(Curve::EaseIn, 0.5), 0.25);
        assert_float_eq!(ease(Curve::EaseOut, 0.5), 0.75);
        assert_float_eq!(ease(Curve::EaseInOut, 0.5), 0.5);
        assert_float_eq!(ease(Curve::EaseInOut, 0.25), 0.125);
    }

    #[test]
    fn cubic_bezier_is_monotonic_and_symmetric() {
        let mut previous = 0.0;
        for i in 1..=100 {
            let value = CA_EASE_IN_OUT.at(i as f32 / 100.0);
            assert!(value >= previous);
            previous = value;
        }
        assert!((CA_EASE_IN_OUT.at(0.5) - 0.5).abs() < 1e-4);
        assert!((CA_EASE_IN_OUT.at(0.3) + CA_EASE_IN_OUT.at(0.7) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn ca_default_is_front_loaded() {
        // (0.25, 0.1, 0.25, 1) reaches ~80 % by the halfway point.
        assert!((CA_DEFAULT.at(0.5) - 0.8024).abs() < 2e-3);
    }
}
