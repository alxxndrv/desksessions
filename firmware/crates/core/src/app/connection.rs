use crate::time::{Instant, Millis};

/// What the BLE adapter reports about the link to the Mac.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkEvent {
    /// No bond: advertising for a Mac to pair.
    Unpaired,
    /// Pairing in progress: show the passkey.
    Passkey(u32),
    /// Bonded Mac connected and encrypted.
    Connected,
    /// Bonded Mac went away.
    Disconnected,
}

/// Where the connection stands, with the grace periods from the spec applied.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Phase {
    #[default]
    Unpaired,
    Pairing,
    Online,
    /// Lost the Mac under 10 s ago: look online, retry quietly.
    Reconnecting,
    /// 10 s – 5 min: dim the list, timers frozen.
    Offline,
    /// Over 5 min: sleepy face.
    Asleep,
}

impl Phase {
    /// The Mac has been gone long enough to show it.
    #[must_use]
    pub fn away(self) -> bool {
        matches!(self, Self::Offline | Self::Asleep)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Connection {
    last: LinkEvent,
    passkey: u32,
    lost_at: Option<Instant>,
}

impl Default for Connection {
    fn default() -> Self {
        Self { last: LinkEvent::Unpaired, passkey: 0, lost_at: None }
    }
}

impl Connection {
    pub const OFFLINE_AFTER: Millis = Millis(10_000);
    pub const ASLEEP_AFTER: Millis = Millis(5 * 60_000);

    pub fn on_event(&mut self, event: LinkEvent, now: Instant) {
        match event {
            LinkEvent::Unpaired | LinkEvent::Connected => self.lost_at = None,
            LinkEvent::Passkey(passkey) => {
                self.passkey = passkey;
                self.lost_at = None;
            }
            LinkEvent::Disconnected => {
                // Repeated reports don't restart the grace period.
                self.lost_at.get_or_insert(now);
            }
        }
        self.last = event;
    }

    #[must_use]
    pub fn phase(&self, now: Instant) -> Phase {
        match self.last {
            LinkEvent::Unpaired => Phase::Unpaired,
            LinkEvent::Passkey(_) => Phase::Pairing,
            LinkEvent::Connected => Phase::Online,
            LinkEvent::Disconnected => {
                let lost = now - self.lost_at.unwrap_or(now);
                if lost >= Self::ASLEEP_AFTER {
                    Phase::Asleep
                } else if lost >= Self::OFFLINE_AFTER {
                    Phase::Offline
                } else {
                    Phase::Reconnecting
                }
            }
        }
    }

    #[must_use]
    pub fn passkey(&self) -> u32 {
        self.passkey
    }

    /// When the Mac was lost, while it is.
    #[must_use]
    pub fn lost_at(&self) -> Option<Instant> {
        self.lost_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_unpaired_then_pairs_and_connects() {
        let mut c = Connection::default();
        let t0 = Instant::from_millis(1_000);
        assert_eq!(c.phase(t0), Phase::Unpaired);
        c.on_event(LinkEvent::Passkey(123_456), t0);
        assert_eq!(c.phase(t0), Phase::Pairing);
        assert_eq!(c.passkey(), 123_456);
        c.on_event(LinkEvent::Connected, t0);
        assert_eq!(c.phase(t0), Phase::Online);
    }

    #[test]
    fn grace_periods_after_losing_the_mac() {
        let mut c = Connection::default();
        let t0 = Instant::from_millis(1_000);
        c.on_event(LinkEvent::Connected, t0);
        c.on_event(LinkEvent::Disconnected, t0);
        assert_eq!(c.phase(t0 + Millis(9_999)), Phase::Reconnecting);
        // A repeated report doesn't restart the clock.
        c.on_event(LinkEvent::Disconnected, t0 + Millis(5_000));
        assert_eq!(c.phase(t0 + Millis(10_000)), Phase::Offline);
        assert_eq!(c.phase(t0 + Millis(300_000)), Phase::Asleep);
        c.on_event(LinkEvent::Connected, t0 + Millis(301_000));
        assert_eq!(c.phase(t0 + Millis(301_000)), Phase::Online);
        assert!(c.lost_at().is_none());
    }
}
