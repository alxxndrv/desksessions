use crate::anim::{Channel, Curve, MOOOK, MOOOK_SOFT, Motion};
use crate::app::Phase;
use crate::time::{Instant, Millis};

const ROW_STAGGER: Millis = Millis(33);
const ROW_COLLAPSE: Millis = MOOOK.duration();
const WAKE_DROP: f32 = 14.0;

const PORT_HOLE: Motion = Motion::tween(Curve::EaseInOut, Millis(198));
const OFFLINE_FADE: Motion = Motion::tween(Curve::CaEaseInOut, Millis(400));
const PAIRING_SLIDE: Motion = Motion::moook(&MOOOK);

fn collapse_length(rows: usize) -> Millis {
    ROW_COLLAPSE + ROW_STAGGER * rows as i64
}

/// Screen-level motion: which screen shows and how far each transition has got. Targets are
/// set from the current phase every frame, so any transition reverses mid-way if the phase
/// flips back.
#[derive(Clone, Debug, Default)]
pub struct StageScene {
    quiet: Channel,
    offline: Channel,
    asleep: Channel,
    pairing: Channel,
    wake: Channel,
    previous: Phase,
    rows: usize,
    offline_since: Instant,
    pairing_since: Instant,
}

impl StageScene {
    /// `rows` sizes the staggered collapse into sleep.
    pub fn update(&mut self, phase: Phase, board_empty: bool, rows: usize, now: Instant) {
        let was_away = self.previous.away();
        if phase == Phase::Online && was_away {
            // Back from being away: the list drops in with Pebble's soft moook.
            self.wake.snap_to(-WAKE_DROP);
            self.wake.animate_to(0.0, Motion::moook(&MOOOK_SOFT), now);
        }
        if phase == Phase::Offline && !self.previous.away() {
            self.offline_since = now;
        }
        if phase == Phase::Pairing && self.previous != Phase::Pairing {
            self.pairing_since = now;
        }
        self.previous = phase;
        if phase != Phase::Asleep {
            self.rows = rows; // keep the count the collapse started with
        }

        let pairing_card = matches!(phase, Phase::Unpaired | Phase::Pairing);
        let flag = |on: bool| if on { 1.0 } else { 0.0 };

        self.quiet.animate_to(flag(board_empty), PORT_HOLE, now);
        self.offline.animate_to(flag(phase.away()), OFFLINE_FADE, now);
        self.asleep.animate_to(
            flag(phase == Phase::Asleep),
            Motion::tween(Curve::Linear, collapse_length(self.rows)),
            now,
        );
        self.pairing.animate_to(flag(pairing_card), PAIRING_SLIDE, now);

        self.quiet.update(now);
        self.offline.update(now);
        self.asleep.update(now);
        self.pairing.update(now);
        self.wake.update(now);
    }

    /// 0 = list, 1 = "all quiet" (port-hole transition between them).
    #[must_use]
    pub fn quiet(&self) -> f32 {
        self.quiet.value()
    }

    /// 0 = live, 1 = offline: rows at half strength, header says the Mac is asleep.
    #[must_use]
    pub fn offline(&self) -> f32 {
        self.offline.value()
    }

    /// 0 = list, 1 = collapsed into the sleepy face. Rows go one after another.
    #[must_use]
    pub fn asleep(&self) -> f32 {
        self.asleep.value()
    }

    /// Collapse progress of row `index`, staggered 33 ms per row.
    #[must_use]
    pub fn collapse(&self, index: usize) -> f32 {
        let total = collapse_length(self.rows).count() as f32;
        let start = (ROW_STAGGER * index as i64).count() as f32;
        let length = ROW_COLLAPSE.count() as f32;
        ((self.asleep.value() * total - start) / length).clamp(0.0, 1.0)
    }

    /// 0 = hidden, 1 = pairing card fully in.
    #[must_use]
    pub fn pairing(&self) -> f32 {
        self.pairing.value()
    }

    /// Vertical offset of the whole list while it bounces awake after a reconnect.
    #[must_use]
    pub fn wake(&self) -> f32 {
        self.wake.value()
    }

    /// When the current offline stretch began, for the bobbing zzz.
    #[must_use]
    pub fn offline_since(&self) -> Instant {
        self.offline_since
    }

    /// When the passkey appeared, for its digits dropping in one by one.
    #[must_use]
    pub fn pairing_since(&self) -> Instant {
        self.pairing_since
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assert_float_eq;

    fn run(
        stage: &mut StageScene,
        phase: Phase,
        empty: bool,
        rows: usize,
        now: &mut Instant,
        ms: i64,
    ) {
        let mut t = 0;
        while t < ms {
            *now += Millis(16);
            stage.update(phase, empty, rows, *now);
            t += 16;
        }
    }

    #[test]
    fn port_hole_to_quiet_reverses_mid_way() {
        let mut stage = StageScene::default();
        let mut now = Instant::from_millis(1_000);
        run(&mut stage, Phase::Online, false, 3, &mut now, 300);
        run(&mut stage, Phase::Online, true, 0, &mut now, 96);
        let midway = stage.quiet();
        assert!(midway > 0.1 && midway < 0.9);
        run(&mut stage, Phase::Online, false, 1, &mut now, 16); // turns around here
        let turned = stage.quiet();
        assert!((turned - midway).abs() < 0.2); // no jump
        run(&mut stage, Phase::Online, false, 1, &mut now, 16);
        assert!(stage.quiet() < turned);
        run(&mut stage, Phase::Online, false, 1, &mut now, 300);
        assert_float_eq!(stage.quiet(), 0.0);
    }

    #[test]
    fn asleep_collapses_rows_in_turn() {
        let mut stage = StageScene::default();
        let mut now = Instant::from_millis(1_000);
        run(&mut stage, Phase::Online, false, 4, &mut now, 100);
        run(&mut stage, Phase::Asleep, false, 4, &mut now, 64);
        assert!(stage.collapse(0) > stage.collapse(1));
        assert!(stage.collapse(1) > stage.collapse(3));
        run(&mut stage, Phase::Asleep, false, 4, &mut now, 600);
        assert_float_eq!(stage.collapse(3), 1.0);
    }

    #[test]
    fn wakes_with_a_bounce_when_the_mac_returns() {
        let mut stage = StageScene::default();
        let mut now = Instant::from_millis(1_000);
        run(&mut stage, Phase::Online, false, 2, &mut now, 100);
        run(&mut stage, Phase::Offline, false, 2, &mut now, 1_000);
        run(&mut stage, Phase::Online, false, 2, &mut now, 16);
        assert!(stage.wake() < 0.0);
        run(&mut stage, Phase::Online, false, 2, &mut now, 600);
        assert_float_eq!(stage.wake(), 0.0);
        assert_float_eq!(stage.offline(), 0.0);
    }
}
