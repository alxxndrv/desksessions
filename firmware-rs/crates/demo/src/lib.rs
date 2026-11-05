//! A pretend Mac: a list of sessions that changes on a loop and reacts to the clock's commands.
//! Drives the simulator and the device's demo build.

#![no_std]

use desk_core::app::MacLink;
use desk_core::domain::{Agent, Command, Session, SessionId, SessionState, Snapshot};
use desk_core::time::{Instant, Millis};

/// Most sessions the pretend Mac keeps (only the first 16 fit in a snapshot).
pub const CAPACITY: usize = 32;

#[derive(Clone, Debug)]
struct Live {
    session: Session,
    since: Instant,
}

struct Seed {
    id: u32,
    agent: Agent,
    state: SessionState,
    name: &'static str,
    detail: &'static str,
    seconds_ago: i64,
}

const SEEDS: [Seed; 5] = [
    Seed {
        id: 1,
        agent: Agent::Claude,
        state: SessionState::NeedsYou,
        name: "Fix flaky checkout test",
        detail: "perm",
        seconds_ago: 42,
    },
    Seed {
        id: 2,
        agent: Agent::Codex,
        state: SessionState::NeedsYou,
        name: "Rate-limit the public API",
        detail: "?",
        seconds_ago: 190,
    },
    Seed {
        id: 3,
        agent: Agent::Claude,
        state: SessionState::Done,
        name: "Write release notes",
        detail: "",
        seconds_ago: 720,
    },
    Seed {
        id: 4,
        agent: Agent::Claude,
        state: SessionState::Working,
        name: "Desk clock firmware",
        detail: "Bash",
        seconds_ago: 65,
    },
    Seed {
        id: 5,
        agent: Agent::Codex,
        state: SessionState::Working,
        name: "Migrate image thumbnails to WebP",
        detail: "",
        seconds_ago: 500,
    },
];

const NEW_NAMES: [&str; 5] = [
    "Onboarding copy pass",
    "Dark mode for settings",
    "landing-page-v2",
    "Миграция базы данных",
    "Audit login flow",
];

/// The scripted Mac.
#[derive(Clone, Debug)]
pub struct Scenario {
    sessions: heapless::Vec<Live, CAPACITY>,
    stash: heapless::Vec<Live, CAPACITY>,
    next_id: u32,
    step: usize,
    step_at: Instant,
    auto: bool,
    dirty: bool,
    forget_requested: bool,
    heard: Option<Command>,
}

impl Scenario {
    #[must_use]
    pub fn new(start: Instant) -> Self {
        let mut scenario = Self {
            sessions: heapless::Vec::new(),
            stash: heapless::Vec::new(),
            next_id: 100,
            step: 0,
            step_at: start,
            auto: true,
            dirty: true,
            forget_requested: false,
            heard: None,
        };
        for seed in &SEEDS {
            let session = Session {
                id: SessionId(seed.id),
                agent: seed.agent,
                state: seed.state,
                name: seed.name.into(),
                detail: seed.detail.into(),
                ..Session::default()
            };
            let _ = scenario
                .sessions
                .push(Live { session, since: start - Millis(seed.seconds_ago * 1000) });
        }
        if let Some(codex) = scenario.find(5) {
            codex.session.subagents = 3;
        }
        scenario
    }

    /// Advances the script. Returns true when the session list changed.
    pub fn advance(&mut self, now: Instant) -> bool {
        if self.auto && now - self.step_at >= Millis(3_500) {
            self.step_at = now;
            let step = self.step % 8;
            self.step += 1;
            match step {
                0 => self.add_session(now),
                1 => self.set_state(4, SessionState::NeedsYou, "perm", now),
                2 => self.set_state(1, SessionState::Working, "Edit", now),
                3 => self.set_state(4, SessionState::Working, "Read", now),
                4 => self.set_state(1, SessionState::Done, "", now),
                5 => self.remove_last(),
                6 => self.set_state(1, SessionState::NeedsYou, "perm", now),
                _ => self.set_state(3, SessionState::Done, "", now),
            }
        }
        core::mem::take(&mut self.dirty)
    }

    #[must_use]
    pub fn snapshot(&self, now: Instant) -> Snapshot {
        let mut snapshot = Snapshot::default();
        for live in &self.sessions {
            let session = Session { in_state: now - live.since, ..live.session.clone() };
            if snapshot.sessions.push(session).is_err() {
                break;
            }
        }
        snapshot
    }

    /// True once after the clock asked to forget its pairing.
    pub fn take_forget_request(&mut self) -> bool {
        core::mem::take(&mut self.forget_requested)
    }

    /// The last command the clock sent, once.
    pub fn take_heard(&mut self) -> Option<Command> {
        self.heard.take()
    }

    pub fn add_session(&mut self, now: Instant) {
        let id = self.next_id;
        let session = Session {
            id: SessionId(id),
            agent: if id.is_multiple_of(2) { Agent::Claude } else { Agent::Codex },
            state: SessionState::Working,
            name: NEW_NAMES[id as usize % NEW_NAMES.len()].into(),
            detail: "Grep".into(),
            ..Session::default()
        };
        self.next_id += 1;
        let _ = self.sessions.push(Live { session, since: now });
        self.dirty = true;
    }

    pub fn remove_last(&mut self) {
        if let Some(index) = self.sessions.iter().rposition(|l| l.session.id.0 >= 100) {
            self.sessions.remove(index);
            self.dirty = true;
        }
    }

    pub fn escalate_first_working(&mut self, now: Instant) {
        if let Some(id) = self
            .sessions
            .iter()
            .find(|l| l.session.state == SessionState::Working)
            .map(|l| l.session.id.0)
        {
            self.set_state(id, SessionState::NeedsYou, "perm", now);
        }
    }

    pub fn toggle_auto(&mut self) {
        self.auto = !self.auto;
    }

    /// Empties the list (everything finished and dismissed), or brings it all back.
    pub fn toggle_empty(&mut self) {
        if self.sessions.is_empty() {
            core::mem::swap(&mut self.sessions, &mut self.stash);
        } else {
            self.stash = self.sessions.clone();
            self.sessions.clear();
        }
        self.dirty = true;
    }

    fn find(&mut self, id: u32) -> Option<&mut Live> {
        self.sessions.iter_mut().find(|l| l.session.id.0 == id)
    }

    fn set_state(&mut self, id: u32, state: SessionState, detail: &str, now: Instant) {
        if self.find(id).is_none() {
            // Dismissed earlier: bring it back as a fresh session.
            let session = Session {
                id: SessionId(id),
                name: "Fix flaky checkout test".into(),
                ..Session::default()
            };
            if self.sessions.push(Live { session, since: Instant::default() }).is_err() {
                return;
            }
        }
        let Some(live) = self.find(id) else { return };
        if live.session.state != state {
            live.since = now;
        }
        live.session.state = state;
        live.session.detail = detail.into();
        self.dirty = true;
    }
}

impl MacLink for Scenario {
    fn send(&mut self, command: Command) {
        self.heard = Some(command);
        if let Command::Dismiss(id) = command {
            self.sessions.retain(|l| l.session.id != id);
            self.dirty = true;
        }
    }

    fn forget_pairing(&mut self) {
        self.forget_requested = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_with_the_seeded_sessions() {
        let start = Instant::from_millis(100_000);
        let mut scenario = Scenario::new(start);
        assert!(scenario.advance(start));
        let snapshot = scenario.snapshot(start);
        assert_eq!(snapshot.sessions.len(), 5);
        assert_eq!(snapshot.sessions[4].subagents, 3);
        assert_eq!(snapshot.sessions[0].in_state, Millis(42_000));
        assert!(!scenario.advance(start));
    }

    #[test]
    fn dismissing_removes_a_session() {
        let start = Instant::from_millis(100_000);
        let mut scenario = Scenario::new(start);
        scenario.advance(start);
        scenario.send(Command::Dismiss(SessionId(3)));
        assert!(scenario.advance(start));
        assert_eq!(scenario.snapshot(start).sessions.len(), 4);
    }
}
