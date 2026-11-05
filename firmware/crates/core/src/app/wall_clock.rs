use crate::time::{Instant, Millis};

/// Local wall-clock time, once known (the Mac sets it on every connect).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WallTime {
    pub hour: u8,
    pub minute: u8,
}

/// Time of day, set by the Mac on connect and carried forward on the monotonic clock.
#[derive(Clone, Copy, Debug, Default)]
pub struct WallClock {
    base: Option<Millis>,
    at: Instant,
}

impl WallClock {
    /// The Mac says it is `seconds_of_day` past local midnight at `now`.
    pub fn set(&mut self, seconds_of_day: u32, now: Instant) {
        self.base = Some(Millis(i64::from(seconds_of_day % 86_400) * 1000));
        self.at = now;
    }

    #[must_use]
    pub fn now(&self, now: Instant) -> Option<WallTime> {
        let base = self.base?;
        let ms = (base.count() + (now - self.at).count()) % 86_400_000;
        let minutes = ms / 60_000;
        Some(WallTime { hour: (minutes / 60) as u8, minute: (minutes % 60) as u8 })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn carries_time_forward() {
        let mut clock = WallClock::default();
        assert!(clock.now(Instant::default()).is_none());
        clock.set(23 * 3600 + 59 * 60 + 30, Instant::from_millis(1_000));
        assert_eq!(clock.now(Instant::from_millis(1_000)), Some(WallTime { hour: 23, minute: 59 }));
        assert_eq!(clock.now(Instant::from_millis(31_000)), Some(WallTime { hour: 0, minute: 0 }));
    }
}
