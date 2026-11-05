use core::f32::consts::PI;

/// Damped spring with unit mass.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpringParams {
    pub stiffness: f32,
    pub damping: f32,
}

/// Apple's perceptual spring parameters (WWDC23 "Animate with springs"): `duration` is the
/// period of the undamped oscillation in seconds, `bounce` is 0 for critical damping and
/// approaches 1 for no damping. Negative bounce (overdamped) is not supported here.
#[must_use]
pub const fn spring_from(duration: f32, bounce: f32) -> SpringParams {
    let omega = 2.0 * PI / duration;
    SpringParams { stiffness: omega * omega, damping: 4.0 * PI * (1.0 - bounce) / duration }
}

// SwiftUI presets (iOS 17 / macOS 14 and later).
pub const SMOOTH: SpringParams = spring_from(0.5, 0.0);
pub const SNAPPY: SpringParams = spring_from(0.5, 0.15);
pub const BOUNCY: SpringParams = spring_from(0.5, 0.30);

/// Fixed physics step. Springs are only ever advanced by this amount so motion is identical on
/// the device, in the simulator and in tests, whatever the frame rate.
pub const SPRING_STEP: f32 = 1.0 / 120.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Spring {
    pub value: f32,
    pub velocity: f32,
    pub target: f32,
}

impl Spring {
    /// Jumps straight to `position` with no motion.
    pub fn snap_to(&mut self, position: f32) {
        self.value = position;
        self.target = position;
        self.velocity = 0.0;
    }

    #[must_use]
    pub fn settled(&self) -> bool {
        const POSITION_TOLERANCE: f32 = 0.01;
        const VELOCITY_TOLERANCE: f32 = 0.05;
        (self.value - self.target).abs() < POSITION_TOLERANCE
            && self.velocity.abs() < VELOCITY_TOLERANCE
    }

    /// Advances by one [`SPRING_STEP`] (semi-implicit Euler).
    pub fn step(&mut self, params: &SpringParams) {
        if self.settled() {
            self.snap_to(self.target);
            return;
        }
        let displacement = self.value - self.target;
        let acceleration = -params.stiffness * displacement - params.damping * self.velocity;
        self.velocity += acceleration * SPRING_STEP;
        self.value += self.velocity * SPRING_STEP;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn steps_to_settle(mut spring: Spring, params: &SpringParams) -> (usize, f32) {
        let mut steps = 0;
        let mut peak = spring.value;
        while !spring.settled() && steps < 10_000 {
            spring.step(params);
            peak = peak.max(spring.value);
            steps += 1;
        }
        (steps, peak)
    }

    fn towards_100() -> Spring {
        Spring { value: 0.0, velocity: 0.0, target: 100.0 }
    }

    #[test]
    fn presets_match_apples_formulas() {
        assert!((SMOOTH.stiffness - 157.91).abs() < 0.01);
        assert!((SMOOTH.damping - 25.13).abs() < 0.01);
        assert!((SNAPPY.damping - 21.36).abs() < 0.01);
        assert!((BOUNCY.damping - 17.59).abs() < 0.01);
    }

    #[test]
    fn smooth_never_overshoots() {
        let (steps, peak) = steps_to_settle(towards_100(), &SMOOTH);
        assert!(peak <= 100.01);
        assert!(steps < 160); // Apple: ~0.8 s to settle
    }

    #[test]
    fn bouncy_overshoots_more_than_snappy() {
        let (_, snappy) = steps_to_settle(towards_100(), &SNAPPY);
        let (_, bouncy) = steps_to_settle(towards_100(), &BOUNCY);
        assert!(snappy > 100.0);
        assert!(bouncy > snappy);
    }

    #[test]
    fn settled_spring_snaps_and_stops() {
        let mut spring = Spring::default();
        spring.snap_to(42.0);
        spring.step(&SNAPPY);
        assert_eq!(spring.value, 42.0);
        assert!(spring.settled());
    }
}
