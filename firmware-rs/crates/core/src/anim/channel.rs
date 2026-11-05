use super::easing::{Curve, ease, lerp};
use super::moook::{MOOOK_OUT, MoookShape, moook};
use super::spring::{BOUNCY, SMOOTH, SPRING_STEP, Spring, SpringParams};
use crate::time::{Instant, Millis, progress};

/// How a channel travels to a new target.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Motion {
    Spring(SpringParams),
    Tween { curve: Curve, duration: Millis },
    Moook(&'static MoookShape),
}

impl Motion {
    #[must_use]
    pub const fn springy(params: SpringParams) -> Self {
        Self::Spring(params)
    }

    #[must_use]
    pub const fn tween(curve: Curve, duration: Millis) -> Self {
        Self::Tween { curve, duration }
    }

    #[must_use]
    pub const fn moook(shape: &'static MoookShape) -> Self {
        Self::Moook(shape)
    }

    /// Whether `self` and `other` are the same kind of motion, whatever their parameters.
    fn same_kind(&self, other: &Self) -> bool {
        core::mem::discriminant(self) == core::mem::discriminant(other)
    }
}

impl Default for Motion {
    fn default() -> Self {
        Self::Spring(SMOOTH)
    }
}

/// Settle-only moook: what an interrupted moook plays from wherever it was (Pebble `menu_layer`).
pub const MOOOK_SETTLE: MoookShape = MoookShape { r#in: &[], out: &MOOOK_OUT, mid_frames: 1 };

/// Longest gap simulated in one update; a hitch longer than this slows motion rather than
/// making springs explode or tweens skip their ending.
const MAX_CATCH_UP: f32 = 0.25;

#[derive(Clone, Copy, Debug)]
struct Leg {
    target: f32,
    motion: Motion,
}

/// One animated value. Every change of target starts from the value currently on screen, with
/// its current velocity, so any motion can be interrupted or reversed without a jump:
///  - springs keep their velocity (Apple);
///  - an interrupted tween continues with ease-out (Pebble `scroll_layer`);
///  - an interrupted moook plays only its settle (Pebble `menu_layer`).
#[derive(Clone, Debug)]
pub struct Channel {
    motion: Motion,
    value: f32,
    velocity: f32,
    from: f32,
    target: f32,
    start: Instant,
    last_update: Instant,
    /// Seconds of spring time not yet simulated.
    step_backlog: f32,
    spring: Spring,
    animating: bool,
    next: Option<Leg>,
}

impl Default for Channel {
    fn default() -> Self {
        Self::new(0.0)
    }
}

impl Channel {
    #[must_use]
    pub fn new(value: f32) -> Self {
        let mut channel = Self {
            motion: Motion::default(),
            value: 0.0,
            velocity: 0.0,
            from: 0.0,
            target: 0.0,
            start: Instant::default(),
            last_update: Instant::default(),
            step_backlog: 0.0,
            spring: Spring::default(),
            animating: false,
            next: None,
        };
        channel.snap_to(value);
        channel
    }

    /// Starts travelling to `target`. Re-requesting the current target with the same kind of
    /// motion leaves the motion in flight alone.
    pub fn animate_to(&mut self, target: f32, motion: Motion, now: Instant) {
        self.update(now);
        self.next = None;
        if self.animating && target == self.target && motion.same_kind(&self.motion) {
            return;
        }
        let interrupted = self.animating;

        self.motion = motion;
        self.from = self.value;
        self.target = target;
        self.start = now;
        self.animating = self.value != target || self.velocity != 0.0;

        match motion {
            Motion::Spring(_) => {
                self.spring = Spring { value: self.value, velocity: self.velocity, target };
                self.step_backlog = 0.0;
            }
            Motion::Tween { duration, .. } => {
                if interrupted {
                    self.motion = Motion::Tween { curve: Curve::EaseOut, duration };
                }
            }
            Motion::Moook(_) => {
                if interrupted {
                    self.motion = Motion::Moook(&MOOOK_SETTLE);
                }
            }
        }
    }

    /// Queues one follow-up leg, started the moment the current motion finishes. For scripted
    /// multi-part gestures (nudge out and back, stretch then jump). Any `animate_to`/`snap_to`
    /// drops it.
    pub fn then(&mut self, target: f32, motion: Motion) {
        self.next = Some(Leg { target, motion });
    }

    /// Jumps to `value` with no motion. For values driven directly, such as a finger's hold.
    pub fn snap_to(&mut self, value: f32) {
        self.next = None;
        self.value = value;
        self.from = value;
        self.target = value;
        self.velocity = 0.0;
        self.spring.snap_to(value);
        self.animating = false;
    }

    /// Adds velocity (units/s) for a spring-driven nudge or pop.
    pub fn kick(&mut self, velocity: f32, now: Instant) {
        self.kick_with(velocity, now, BOUNCY);
    }

    /// [`Channel::kick`] with a chosen spring.
    pub fn kick_with(&mut self, velocity: f32, now: Instant, params: SpringParams) {
        self.update(now);
        let target = if self.animating { self.target } else { self.value };
        self.velocity += velocity;
        self.animate_to(target, Motion::Spring(params), now);
        self.spring.velocity = self.velocity;
        self.animating = true;
    }

    /// Advances to `now`. Call once per frame before reading.
    pub fn update(&mut self, now: Instant) {
        let dt = MAX_CATCH_UP.min((now - self.last_update).max(Millis::ZERO).as_secs_f32());
        self.last_update = now;
        if !self.animating {
            return;
        }

        match self.motion {
            Motion::Spring(params) => {
                self.step_backlog += dt;
                while self.step_backlog >= SPRING_STEP {
                    self.spring.step(&params);
                    self.step_backlog -= SPRING_STEP;
                }
                self.value = self.spring.value;
                self.velocity = self.spring.velocity;
                if self.spring.settled() {
                    self.finish(now);
                }
            }
            Motion::Tween { duration, .. } => self.advance_timed(now, duration),
            Motion::Moook(shape) => self.advance_timed(now, shape.duration()),
        }
    }

    fn advance_timed(&mut self, now: Instant, length: Millis) {
        if now - self.start >= length {
            self.finish(now);
            return;
        }
        self.value = self.sample(now);
        // Velocity over the last millisecond, so a spring taking over carries the momentum on.
        self.velocity = (self.value - self.sample(now - Millis(1))) * 1000.0;
    }

    fn finish(&mut self, now: Instant) {
        self.value = self.target;
        self.from = self.target;
        self.velocity = 0.0;
        self.spring.snap_to(self.target);
        self.animating = false;
        if let Some(leg) = self.next {
            self.animate_to(leg.target, leg.motion, now);
        }
    }

    fn sample(&self, at: Instant) -> f32 {
        let elapsed = (at - self.start).max(Millis::ZERO);
        match self.motion {
            Motion::Moook(shape) => moook(shape, self.from, self.target, elapsed),
            Motion::Tween { curve, duration } => {
                lerp(self.from, self.target, ease(curve, progress(elapsed, duration)))
            }
            Motion::Spring(_) => self.value,
        }
    }

    #[must_use]
    pub fn value(&self) -> f32 {
        self.value
    }

    #[must_use]
    pub fn velocity(&self) -> f32 {
        self.velocity
    }

    #[must_use]
    pub fn target(&self) -> f32 {
        self.target
    }

    #[must_use]
    pub fn animating(&self) -> bool {
        self.animating
    }
}

#[cfg(test)]
mod tests {
    use super::super::easing::Curve;
    use super::super::moook::MOOOK;
    use super::super::spring::{SMOOTH, SNAPPY};
    use super::*;
    use crate::assert_float_eq;

    /// Runs `channel` frame by frame at 60 fps for `ms`, returning the largest jump between
    /// frames.
    fn run(channel: &mut Channel, now: &mut Instant, ms: i64) -> f32 {
        let mut largest = 0.0_f32;
        let mut previous = channel.value();
        let mut t = 0;
        while t < ms {
            *now += Millis(16);
            channel.update(*now);
            largest = largest.max((channel.value() - previous).abs());
            previous = channel.value();
            t += 16;
        }
        largest
    }

    fn start() -> Instant {
        Instant::from_millis(1_000)
    }

    #[test]
    fn tween_reaches_target_exactly() {
        let mut now = start();
        let mut channel = Channel::new(0.0);
        channel.animate_to(100.0, Motion::tween(Curve::EaseInOut, Millis(250)), now);
        run(&mut channel, &mut now, 300);
        assert_eq!(channel.value(), 100.0);
        assert!(!channel.animating());
    }

    #[test]
    fn reversing_a_tween_mid_flight_does_not_jump() {
        let mut now = start();
        let mut channel = Channel::new(0.0);
        channel.animate_to(100.0, Motion::tween(Curve::EaseInOut, Millis(250)), now);
        run(&mut channel, &mut now, 112);
        let before = channel.value();
        assert!(before > 10.0 && before < 90.0);

        channel.animate_to(0.0, Motion::tween(Curve::EaseInOut, Millis(250)), now);
        assert_eq!(channel.value(), before);
        // Pebble: an interrupted scroll continues with ease-out, so it moves off quickly.
        let largest = run(&mut channel, &mut now, 300);
        assert_eq!(channel.value(), 0.0);
        assert!(largest < 20.0);
    }

    #[test]
    fn interrupted_moook_only_settles() {
        let mut now = start();
        let mut channel = Channel::new(0.0);
        channel.animate_to(30.0, Motion::moook(&MOOOK), now);
        run(&mut channel, &mut now, 48);
        let before = channel.value();

        // Second press mid-flight: next row down.
        channel.animate_to(60.0, Motion::moook(&MOOOK), now);
        assert_eq!(channel.value(), before);
        now += Millis(1);
        channel.update(now);
        assert!(channel.value() > before); // no anticipation backwards
        run(&mut channel, &mut now, MOOOK_SETTLE.duration().count() + 32);
        assert_eq!(channel.value(), 60.0);
    }

    #[test]
    fn retargeting_a_spring_keeps_momentum() {
        let mut now = start();
        let mut channel = Channel::new(0.0);
        channel.animate_to(100.0, Motion::springy(SNAPPY), now);
        run(&mut channel, &mut now, 100);
        let velocity = channel.velocity();
        assert!(velocity > 0.0);

        channel.animate_to(200.0, Motion::springy(SNAPPY), now);
        assert_eq!(channel.velocity(), velocity);
        run(&mut channel, &mut now, 2_000);
        assert!((channel.value() - 200.0).abs() < 0.01);
    }

    #[test]
    fn spring_takes_over_a_tweens_momentum() {
        let mut now = start();
        let mut channel = Channel::new(0.0);
        channel.animate_to(100.0, Motion::tween(Curve::Linear, Millis(1_000)), now);
        run(&mut channel, &mut now, 496);
        assert!((channel.velocity() - 100.0).abs() < 1.0);

        channel.animate_to(0.0, Motion::springy(SMOOTH), now);
        assert!((channel.velocity() - 100.0).abs() < 1.0);
        let before = channel.value();
        now += Millis(9); // one physics step
        channel.update(now);
        assert!(channel.value() > before); // still coasting forward before turning back
    }

    #[test]
    fn same_target_does_not_restart() {
        let mut now = start();
        let mut channel = Channel::new(0.0);
        let motion = Motion::tween(Curve::Linear, Millis(100));
        channel.animate_to(10.0, motion, now);
        run(&mut channel, &mut now, 48);
        channel.animate_to(10.0, motion, now);
        run(&mut channel, &mut now, 64);
        assert!(!channel.animating());
    }

    #[test]
    fn kick_pops_and_returns() {
        let mut now = start();
        let mut scale = Channel::new(1.0);
        scale.kick(1.5, now);
        run(&mut scale, &mut now, 80);
        assert!(scale.value() > 1.02);
        run(&mut scale, &mut now, 2_000);
        assert!((scale.value() - 1.0).abs() < 0.01);
    }

    #[test]
    fn then_runs_a_follow_up_leg() {
        let mut now = start();
        let mut nudge = Channel::new(0.0);
        nudge.animate_to(-5.0, Motion::tween(Curve::EaseOut, Millis(48)), now);
        nudge.then(0.0, Motion::tween(Curve::EaseOut, Millis(144)));
        run(&mut nudge, &mut now, 48);
        assert_float_eq!(nudge.value(), -5.0);
        assert!(nudge.animating());
        run(&mut nudge, &mut now, 160);
        assert_eq!(nudge.value(), 0.0);
        assert!(!nudge.animating());
    }

    #[test]
    fn interruption_drops_the_follow_up() {
        let mut now = start();
        let mut channel = Channel::new(0.0);
        channel.animate_to(10.0, Motion::tween(Curve::Linear, Millis(100)), now);
        channel.then(50.0, Motion::tween(Curve::Linear, Millis(100)));
        run(&mut channel, &mut now, 32);
        channel.animate_to(-10.0, Motion::tween(Curve::Linear, Millis(100)), now);
        run(&mut channel, &mut now, 400);
        assert_eq!(channel.value(), -10.0);
    }
}
