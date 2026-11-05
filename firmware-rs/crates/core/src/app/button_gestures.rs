use crate::time::{Instant, Millis, progress};

/// BOOT (GPIO0) moves the cursor up; Aux (GPIO21) moves it down.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Boot,
    Aux,
}

impl Button {
    const ALL: [Self; 2] = [Self::Boot, Self::Aux];

    const fn index(self) -> usize {
        match self {
            Self::Boot => 0,
            Self::Aux => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GestureKind {
    /// Finger landed: immediate feedback.
    Down,
    /// Released before the long-press threshold.
    Press,
    /// Held past the threshold (fires while still held).
    LongPress,
    /// Both buttons held for the re-pair time.
    Chord,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gesture {
    pub kind: GestureKind,
    /// Meaningless for [`GestureKind::Chord`].
    pub button: Button,
}

/// Progress of a long press in flight, for the fill that sweeps across the row.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hold {
    pub button: Button,
    /// 0 → 1 over the long-press threshold.
    pub progress: f32,
}

#[derive(Clone, Copy, Debug, Default)]
struct Key {
    raw: bool,
    stable: bool,
    raw_since: Instant,
    down_since: Instant,
    long_fired: bool,
}

impl Key {
    /// Debounces one key. Returns true when its stable level changed.
    fn debounce(&mut self, raw: bool, now: Instant) -> bool {
        if raw != self.raw {
            self.raw = raw;
            self.raw_since = now;
        }
        if self.raw != self.stable && now - self.raw_since >= ButtonGestures::DEBOUNCE {
            self.stable = self.raw;
            return true;
        }
        false
    }
}

/// Turns raw button levels into gestures. Fed once per tick with the current levels.
#[derive(Clone, Debug, Default)]
pub struct ButtonGestures {
    keys: [Key; 2],
    chording: bool,
    chord_fired: bool,
    chord_since: Instant,
}

impl ButtonGestures {
    pub const DEBOUNCE: Millis = Millis(15);
    pub const LONG_PRESS: Millis = Millis(600);
    pub const CHORD: Millis = Millis(5_000);

    /// `boot_down` / `aux_down` are raw levels (true = pressed). Returns at most one gesture.
    pub fn update(&mut self, boot_down: bool, aux_down: bool, now: Instant) -> Option<Gesture> {
        let mut gesture = None;
        if self.keys[Button::Boot.index()].debounce(boot_down, now) {
            gesture = self.on_edge(Button::Boot, now);
        }
        if self.keys[Button::Aux.index()].debounce(aux_down, now)
            && let Some(aux) = self.on_edge(Button::Aux, now)
        {
            gesture = Some(aux);
        }
        if gesture.is_some() {
            return gesture;
        }

        if self.chording {
            if !self.chord_fired && now - self.chord_since >= Self::CHORD {
                self.chord_fired = true;
                return Some(Gesture { kind: GestureKind::Chord, button: Button::Boot });
            }
            return None;
        }

        for button in Button::ALL {
            let key = &mut self.keys[button.index()];
            if key.stable && !key.long_fired && now - key.down_since >= Self::LONG_PRESS {
                key.long_fired = true;
                return Some(Gesture { kind: GestureKind::LongPress, button });
            }
        }
        None
    }

    fn on_edge(&mut self, button: Button, now: Instant) -> Option<Gesture> {
        let other_stable = self.keys[1 - button.index()].stable;
        let key = &mut self.keys[button.index()];

        if key.stable {
            key.down_since = now;
            key.long_fired = false;
            if other_stable {
                // Second finger down: from now on this is a chord, not two presses.
                self.chording = true;
                self.chord_fired = false;
                self.chord_since = now;
                return None;
            }
            return Some(Gesture { kind: GestureKind::Down, button });
        }

        if self.chording {
            if !other_stable {
                self.chording = false; // both released
            }
            key.long_fired = true; // the remaining finger's release must not count as a press
            return None;
        }
        if key.long_fired {
            return None;
        }
        Some(Gesture { kind: GestureKind::Press, button })
    }

    /// The long press currently filling up, if any.
    #[must_use]
    pub fn hold(&self, now: Instant) -> Option<Hold> {
        if self.chording {
            return None;
        }
        Button::ALL.into_iter().find_map(|button| {
            let key = &self.keys[button.index()];
            (key.stable && !key.long_fired).then(|| Hold {
                button,
                progress: progress(now - key.down_since, Self::LONG_PRESS),
            })
        })
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::vec::Vec;

    use super::*;
    use GestureKind::{Chord, Down, LongPress, Press};

    /// Drives the classifier one millisecond at a time and records every gesture.
    #[derive(Default)]
    struct Fingers {
        gestures: ButtonGestures,
        now: Instant,
        seen: Vec<Gesture>,
    }

    impl Fingers {
        fn hold(&mut self, boot: bool, aux: bool, ms: i64) {
            for _ in 0..ms {
                self.now += Millis(1);
                if let Some(g) = self.gestures.update(boot, aux, self.now) {
                    self.seen.push(g);
                }
            }
        }

        fn hold_state(&self) -> Option<Hold> {
            self.gestures.hold(self.now)
        }
    }

    fn g(kind: GestureKind, button: Button) -> Gesture {
        Gesture { kind, button }
    }

    #[test]
    fn short_press_gives_down_then_press() {
        let mut f = Fingers::default();
        f.hold(false, false, 10);
        f.hold(true, false, 100);
        f.hold(false, false, 50);
        assert_eq!(f.seen, [g(Down, Button::Boot), g(Press, Button::Boot)]);
    }

    #[test]
    fn ignores_bounces() {
        let mut f = Fingers::default();
        for _ in 0..5 {
            f.hold(false, true, 3);
            f.hold(false, false, 3);
        }
        assert!(f.seen.is_empty());
    }

    #[test]
    fn long_press_fires_while_held_and_release_is_silent() {
        let mut f = Fingers::default();
        f.hold(false, true, 400);
        let hold = f.hold_state().unwrap();
        assert_eq!(hold.button, Button::Aux);
        assert!((hold.progress - (400.0 - 15.0) / 600.0).abs() < 0.01);

        f.hold(false, true, 300);
        assert_eq!(f.seen, [g(Down, Button::Aux), g(LongPress, Button::Aux)]);
        assert!(f.hold_state().is_none());

        f.hold(false, false, 50);
        assert_eq!(f.seen.len(), 2);
    }

    #[test]
    fn both_buttons_for_five_seconds_is_a_chord_only() {
        let mut f = Fingers::default();
        f.hold(true, false, 100);
        f.hold(true, true, 5_100);
        f.hold(true, false, 100);
        f.hold(false, false, 100);
        assert_eq!(f.seen, [g(Down, Button::Boot), g(Chord, Button::Boot)]);
    }

    #[test]
    fn abandoned_chord_produces_nothing() {
        let mut f = Fingers::default();
        f.hold(false, true, 50);
        f.hold(true, true, 1_000);
        f.hold(false, false, 100);
        assert_eq!(f.seen, [g(Down, Button::Aux)]);
    }
}
