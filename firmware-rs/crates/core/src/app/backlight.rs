use super::connection::Phase;
use super::wall_clock::WallTime;
use crate::anim::{BREATH_PERIOD, Channel, Curve, Motion, breathe};
use crate::domain::Alert;
use crate::time::{Instant, Millis};

/// Quiet hours: the backlight stays low and alerts don't pulse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NightHours {
    pub start_hour: u8,
    pub end_hour: u8,
}

impl Default for NightHours {
    fn default() -> Self {
        Self { start_hour: 23, end_hour: 8 }
    }
}

impl NightHours {
    #[must_use]
    pub fn contains(&self, t: WallTime) -> bool {
        if self.start_hour <= self.end_hour {
            t.hour >= self.start_hour && t.hour < self.end_hour
        } else {
            t.hour >= self.start_hour || t.hour < self.end_hour
        }
    }
}

const PULSE_DEPTH: f32 = 0.65;
const SETTLE: Motion = Motion::tween(Curve::CaEaseInOut, Millis(600));
const FOLLOW: Motion = Motion::tween(Curve::EaseOut, Millis(120));

fn pulses_for(alert: Alert) -> i64 {
    match alert {
        Alert::NeedsYou => 3,
        Alert::Done => 1,
        Alert::None => 0,
    }
}

/// Decides how bright the screen is: a resting level for the situation, dipped by alert pulses.
#[derive(Clone, Debug)]
pub struct BacklightController {
    night: NightHours,
    day: f32,
    night_level: f32,
    /// The user is dragging a slider: track it closely.
    follow_hand: bool,
    resting: Channel,
    night_active: bool,
    pulse_start: Option<Instant>,
    pulses: i64,
}

impl Default for BacklightController {
    fn default() -> Self {
        Self {
            night: NightHours::default(),
            day: 1.0,
            night_level: 0.10,
            follow_hand: false,
            resting: Channel::new(1.0),
            night_active: false,
            pulse_start: None,
            pulses: 0,
        }
    }
}

impl BacklightController {
    /// Empty board: this share of the day level.
    pub const QUIET_FACTOR: f32 = 0.6;
    /// Mac away for minutes: at most this.
    pub const ASLEEP: f32 = 0.15;
    pub const MERGE_WINDOW: Millis = Millis(5_000);

    pub fn alert(&mut self, alert: Alert, now: Instant) {
        let pulses = pulses_for(alert);
        if pulses == 0 || self.night_active {
            return;
        }
        if let Some(start) = self.pulse_start {
            let elapsed = now - start;
            if elapsed < Self::MERGE_WINDOW {
                // A burst: one sequence, at the strongest alert.
                self.pulses = self.pulses.max(pulses);
                return;
            }
            if elapsed < BREATH_PERIOD * self.pulses {
                // Still pulsing: carry on from the current breath instead of restarting mid-dip.
                let current = elapsed.div(BREATH_PERIOD);
                self.pulses = self.pulses.max(current + pulses);
                return;
            }
        }
        self.pulse_start = Some(now);
        self.pulses = pulses;
    }

    /// Brightness in [0, 1] for this frame.
    pub fn level(
        &mut self,
        phase: Phase,
        board_empty: bool,
        wall_time: Option<WallTime>,
        now: Instant,
    ) -> f32 {
        self.night_active = wall_time.is_some_and(|t| self.night.contains(t));

        let mut target = self.day;
        if phase == Phase::Asleep {
            target = self.day.min(Self::ASLEEP);
        } else if board_empty && phase != Phase::Unpaired && phase != Phase::Pairing {
            target = self.day * Self::QUIET_FACTOR;
        }
        if self.night_active {
            target = target.min(self.night_level);
        }
        if self.follow_hand {
            self.resting.animate_to(target, FOLLOW, now);
            self.follow_hand = false;
        } else if target != self.resting.target() {
            self.resting.animate_to(target, SETTLE, now);
        }
        self.resting.update(now);
        self.resting.value() * (1.0 - self.pulse_dip(now))
    }

    pub fn set_night_hours(&mut self, hours: NightHours) {
        self.night = hours;
    }

    /// Levels chosen in the Mac app, in percent.
    pub fn set_brightness(&mut self, day: u8, night: u8) {
        let next_day = f32::from(day) / 100.0;
        let next_night = f32::from(night) / 100.0;
        self.follow_hand =
            self.follow_hand || next_day != self.day || next_night != self.night_level;
        self.day = next_day;
        self.night_level = next_night;
    }

    #[must_use]
    pub fn day(&self) -> f32 {
        self.day
    }

    #[must_use]
    pub fn night(&self) -> f32 {
        self.night_level
    }

    fn pulse_dip(&self, now: Instant) -> f32 {
        let Some(start) = self.pulse_start else {
            return 0.0;
        };
        if self.night_active {
            return 0.0;
        }
        let elapsed = now - start;
        if elapsed >= BREATH_PERIOD * self.pulses {
            return 0.0;
        }
        PULSE_DEPTH * breathe(elapsed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assert_float_eq;

    const NOON: WallTime = WallTime { hour: 12, minute: 0 };
    const MIDNIGHT: WallTime = WallTime { hour: 0, minute: 30 };

    fn settle(
        b: &mut BacklightController,
        phase: Phase,
        empty: bool,
        t: WallTime,
        now: &mut Instant,
    ) -> f32 {
        let mut level = 0.0;
        let mut i = 0;
        while i < 1_000 {
            *now += Millis(16);
            level = b.level(phase, empty, Some(t), *now);
            i += 16;
        }
        level
    }

    fn start() -> Instant {
        Instant::from_millis(1_000)
    }

    #[test]
    fn resting_levels_by_situation() {
        let mut b = BacklightController::default();
        let mut now = start();
        assert_float_eq!(settle(&mut b, Phase::Online, false, NOON, &mut now), 1.0);
        assert_float_eq!(
            settle(&mut b, Phase::Online, true, NOON, &mut now),
            BacklightController::QUIET_FACTOR
        );
        assert_float_eq!(
            settle(&mut b, Phase::Asleep, false, NOON, &mut now),
            BacklightController::ASLEEP
        );
        assert_float_eq!(settle(&mut b, Phase::Online, false, MIDNIGHT, &mut now), 0.10);
    }

    #[test]
    fn follows_chosen_brightness() {
        let mut b = BacklightController::default();
        let mut now = start();
        b.set_brightness(40, 5);
        assert_float_eq!(settle(&mut b, Phase::Online, false, NOON, &mut now), 0.40);
        assert_float_eq!(
            settle(&mut b, Phase::Online, true, NOON, &mut now),
            0.40 * BacklightController::QUIET_FACTOR
        );
        assert_float_eq!(settle(&mut b, Phase::Online, false, MIDNIGHT, &mut now), 0.05);
    }

    #[test]
    fn needs_you_pulses_three_times() {
        let mut b = BacklightController::default();
        let mut now = start();
        settle(&mut b, Phase::Online, false, NOON, &mut now);
        b.alert(Alert::NeedsYou, now);
        let begin = now;
        let mut dips = 0;
        let mut in_dip = false;
        while now - begin < Millis(7_000) {
            now += Millis(16);
            let dipping = b.level(Phase::Online, false, Some(NOON), now) < 0.6;
            if dipping && !in_dip {
                dips += 1;
            }
            in_dip = dipping;
        }
        assert_eq!(dips, 3);
    }

    #[test]
    fn bursts_merge_into_one_sequence() {
        let mut b = BacklightController::default();
        let mut now = start();
        settle(&mut b, Phase::Online, false, NOON, &mut now);
        b.alert(Alert::Done, now);
        now += Millis(1_000);
        b.alert(Alert::Done, now); // within 5 s: does not restart
        now += Millis(1_000); // 2 s after the first: its single pulse is over
        assert_float_eq!(b.level(Phase::Online, false, Some(NOON), now), 1.0);
    }

    #[test]
    fn late_alerts_extend_pulsing_without_a_jump() {
        let mut b = BacklightController::default();
        let mut now = start();
        settle(&mut b, Phase::Online, false, NOON, &mut now);
        b.alert(Alert::NeedsYou, now);
        let mut previous = b.level(Phase::Online, false, Some(NOON), now);
        let mut largest = 0.0_f32;
        let mut t = 0;
        while t < 12_000 {
            now += Millis(16);
            if t == 5_200 {
                b.alert(Alert::NeedsYou, now); // past the merge window, mid third breath
            }
            let level = b.level(Phase::Online, false, Some(NOON), now);
            largest = largest.max((level - previous).abs());
            previous = level;
            t += 16;
        }
        assert!(largest < 0.05);
    }

    #[test]
    fn no_pulses_at_night() {
        let mut b = BacklightController::default();
        let mut now = start();
        settle(&mut b, Phase::Online, false, MIDNIGHT, &mut now);
        b.alert(Alert::NeedsYou, now);
        now += Millis(900);
        assert_float_eq!(b.level(Phase::Online, false, Some(MIDNIGHT), now), 0.10);
    }
}
