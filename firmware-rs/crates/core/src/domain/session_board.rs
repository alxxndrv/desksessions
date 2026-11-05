use super::{Command, MAX_SESSIONS, Session, SessionId, SessionState, Snapshot};
use crate::time::{Instant, Millis};

/// Something that should catch the eye after a snapshot. Ordered by strength.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Alert {
    #[default]
    None,
    Done,
    NeedsYou,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StateCounts {
    pub needs_you: u8,
    pub done: u8,
    pub working: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CursorMove {
    Up,
    Down,
}

/// Result of moving the cursor: whether it wrapped past an end of the list.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CursorMoved {
    pub moved: bool,
    pub wrapped: bool,
}

/// The aggregate behind the clock: the sorted list of visible sessions plus the cursor.
///
/// Invariants:
///  - rows are sorted NEEDS YOU → DONE → WORKING, longest-waiting first within a state;
///  - the cursor stays on the same session across snapshots while that session is visible;
///  - a dismissed DONE session stays hidden until the Mac reports it in another state.
#[derive(Clone, Debug, Default)]
pub struct SessionBoard {
    rows: heapless::Vec<Session, MAX_SESSIONS>,
    dismissed: heapless::Vec<SessionId, MAX_SESSIONS>,
    cursor: usize,
    received_at: Instant,
    frozen_at: Option<Instant>,
}

fn sorts_before(a: &Session, b: &Session) -> core::cmp::Ordering {
    a.state
        .cmp(&b.state)
        .then_with(|| b.in_state.cmp(&a.in_state)) // longest-waiting first
        .then_with(|| a.id.0.cmp(&b.id.0))
}

fn alert_for(now: &Session, before: Option<&Session>) -> Alert {
    let changed = before.is_none_or(|b| b.state != now.state);
    if !changed {
        return Alert::None;
    }
    match now.state {
        SessionState::NeedsYou => Alert::NeedsYou,
        SessionState::Done => Alert::Done,
        SessionState::Working => Alert::None,
    }
}

impl SessionBoard {
    /// Replaces the board's contents. `now` timestamps the snapshot so time-in-state keeps
    /// counting locally between snapshots. Returns the strongest alert the change deserves.
    pub fn apply(&mut self, snapshot: &Snapshot, now: Instant) -> Alert {
        self.forget_stale_dismissals(snapshot);

        let selected_id = self.selected().map(|s| s.id);

        let mut next: heapless::Vec<Session, MAX_SESSIONS> = heapless::Vec::new();
        let mut alert = Alert::None;
        for session in &snapshot.sessions {
            if self.is_dismissed(session) {
                continue;
            }
            let before = self.rows.iter().find(|r| r.id == session.id);
            alert = alert.max(alert_for(session, before));
            // Cannot fail: a snapshot holds at most MAX_SESSIONS.
            let _ = next.push(session.clone());
        }
        next.sort_unstable_by(sorts_before);

        self.rows = next;
        self.received_at = now;
        self.frozen_at = None;

        if let Some(id) = selected_id
            && let Some(index) = self.rows.iter().position(|s| s.id == id)
        {
            self.cursor = index;
            return alert;
        }
        self.cursor = if self.rows.is_empty() { 0 } else { self.cursor.min(self.rows.len() - 1) };
        alert
    }

    #[must_use]
    pub fn rows(&self) -> &[Session] {
        &self.rows
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    #[must_use]
    pub fn counts(&self) -> StateCounts {
        let mut counts = StateCounts::default();
        for row in &self.rows {
            match row.state {
                SessionState::NeedsYou => counts.needs_you += 1,
                SessionState::Done => counts.done += 1,
                SessionState::Working => counts.working += 1,
            }
        }
        counts
    }

    #[must_use]
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    #[must_use]
    pub fn selected(&self) -> Option<&Session> {
        self.rows.get(self.cursor)
    }

    pub fn move_cursor(&mut self, direction: CursorMove) -> CursorMoved {
        if self.rows.len() < 2 {
            return CursorMoved::default();
        }
        let last = self.rows.len() - 1;
        let wrapped = match direction {
            CursorMove::Up => {
                let wrapped = self.cursor == 0;
                self.cursor = if wrapped { last } else { self.cursor - 1 };
                wrapped
            }
            CursorMove::Down => {
                let wrapped = self.cursor == last;
                self.cursor = if wrapped { 0 } else { self.cursor + 1 };
                wrapped
            }
        };
        CursorMoved { moved: true, wrapped }
    }

    pub fn reset_cursor(&mut self) {
        self.cursor = 0;
    }

    /// Hides the selected session if it is DONE. Returns the command to tell the Mac, or nothing
    /// when the selected row cannot be dismissed.
    pub fn dismiss_selected(&mut self) -> Option<Command> {
        let row = self.selected()?;
        if row.state != SessionState::Done || self.dismissed.is_full() {
            return None;
        }
        let id = row.id;
        let _ = self.dismissed.push(id);
        self.rows.remove(self.cursor);
        self.cursor = if self.rows.is_empty() { 0 } else { self.cursor.min(self.rows.len() - 1) };
        Some(Command::Dismiss(id))
    }

    #[must_use]
    pub fn jump_to_selected(&self) -> Option<Command> {
        self.selected().map(|row| Command::Jump(row.id))
    }

    /// Time `row` has spent in its state at `now`, counting locally since the snapshot arrived.
    #[must_use]
    pub fn time_in_state(&self, row: &Session, now: Instant) -> Millis {
        let until = self.frozen_at.unwrap_or(now);
        row.in_state + (until - self.received_at).max(Millis::ZERO)
    }

    /// Stops time-in-state from advancing (the Mac is unreachable) or resumes it.
    pub fn freeze_time(&mut self, at: Option<Instant>) {
        self.frozen_at = at;
    }

    fn is_dismissed(&self, session: &Session) -> bool {
        session.state == SessionState::Done && self.dismissed.contains(&session.id)
    }

    fn forget_stale_dismissals(&mut self, snapshot: &Snapshot) {
        // A dismissal lasts while the Mac still reports that session as DONE.
        self.dismissed.retain(|id| {
            snapshot
                .sessions
                .iter()
                .find(|s| s.id == *id)
                .is_some_and(|s| s.state == SessionState::Done)
        });
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::vec::Vec;

    use super::*;

    fn session(id: u32, state: SessionState, seconds_in_state: i64) -> Session {
        Session {
            id: SessionId(id),
            state,
            name: "session".into(),
            in_state: Millis(seconds_in_state * 1000),
            ..Session::default()
        }
    }

    fn snapshot_of(sessions: &[Session]) -> Snapshot {
        Snapshot { sessions: sessions.iter().cloned().collect() }
    }

    fn ids(board: &SessionBoard) -> Vec<u32> {
        board.rows().iter().map(|r| r.id.0).collect()
    }

    const T0: Instant = Instant::from_millis(1_000);

    use SessionState::{Done, NeedsYou, Working};

    #[test]
    fn sorts_by_urgency_then_longest_waiting() {
        let mut board = SessionBoard::default();
        board.apply(
            &snapshot_of(&[
                session(1, Working, 50),
                session(2, Done, 10),
                session(3, NeedsYou, 5),
                session(4, Done, 90),
                session(5, NeedsYou, 60),
            ]),
            T0,
        );
        assert_eq!(ids(&board), [5, 3, 4, 2, 1]);
        assert_eq!(board.counts(), StateCounts { needs_you: 2, done: 2, working: 1 });
    }

    #[test]
    fn cursor_follows_the_same_session_across_reorders() {
        let mut board = SessionBoard::default();
        board.apply(&snapshot_of(&[session(1, Done, 0), session(2, Working, 0)]), T0);
        board.move_cursor(CursorMove::Down);
        assert_eq!(board.selected().unwrap().id.0, 2);

        // Session 7 jumps in above it: the cursor index changes, the selection does not.
        board.apply(
            &snapshot_of(&[session(7, NeedsYou, 0), session(1, Done, 0), session(2, Working, 0)]),
            T0,
        );
        assert_eq!(board.cursor(), 2);
        assert_eq!(board.selected().unwrap().id.0, 2);
    }

    #[test]
    fn cursor_clamps_when_its_session_disappears() {
        let mut board = SessionBoard::default();
        board.apply(&snapshot_of(&[session(1, Done, 0), session(2, Working, 0)]), T0);
        board.move_cursor(CursorMove::Down);
        board.apply(&snapshot_of(&[session(1, Done, 0)]), T0);
        assert_eq!(board.cursor(), 0);
        board.apply(&Snapshot::default(), T0);
        assert!(board.selected().is_none());
    }

    #[test]
    fn cursor_wraps_and_reports_it() {
        let mut board = SessionBoard::default();
        board.apply(
            &snapshot_of(&[session(1, Done, 0), session(2, Done, 0), session(3, Done, 0)]),
            T0,
        );
        assert!(board.move_cursor(CursorMove::Up).wrapped);
        assert_eq!(board.cursor(), 2);
        let down = board.move_cursor(CursorMove::Down);
        assert!(down.moved);
        assert!(down.wrapped);
        assert_eq!(board.cursor(), 0);
        assert!(!board.move_cursor(CursorMove::Down).wrapped);
    }

    #[test]
    fn single_row_cursor_does_not_move() {
        let mut board = SessionBoard::default();
        board.apply(&snapshot_of(&[session(1, Done, 0)]), T0);
        assert!(!board.move_cursor(CursorMove::Down).moved);
    }

    #[test]
    fn alerts_only_on_entering_attention_states() {
        let mut board = SessionBoard::default();
        assert_eq!(board.apply(&snapshot_of(&[session(1, Working, 0)]), T0), Alert::None);
        assert_eq!(board.apply(&snapshot_of(&[session(1, Done, 0)]), T0), Alert::Done);
        assert_eq!(board.apply(&snapshot_of(&[session(1, Done, 0)]), T0), Alert::None);
        assert_eq!(
            board.apply(&snapshot_of(&[session(1, Done, 0), session(2, NeedsYou, 0)]), T0),
            Alert::NeedsYou
        );
    }

    #[test]
    fn dismiss_hides_done_until_it_changes_state() {
        let mut board = SessionBoard::default();
        board.apply(&snapshot_of(&[session(1, Done, 0), session(2, Working, 0)]), T0);

        assert_eq!(board.dismiss_selected(), Some(Command::Dismiss(SessionId(1))));
        assert_eq!(ids(&board), [2]);

        // The Mac hasn't processed the dismissal yet and still reports it DONE: stays hidden.
        board.apply(&snapshot_of(&[session(1, Done, 0), session(2, Working, 0)]), T0);
        assert_eq!(ids(&board), [2]);

        // You prompted it again: it is back.
        board.apply(&snapshot_of(&[session(1, Working, 0), session(2, Working, 0)]), T0);
        assert_eq!(ids(&board).len(), 2);

        // Done again later: a fresh DONE shows and alerts.
        assert_eq!(board.apply(&snapshot_of(&[session(1, Done, 0)]), T0), Alert::Done);
        assert_eq!(ids(&board), [1]);
    }

    #[test]
    fn only_done_rows_can_be_dismissed() {
        let mut board = SessionBoard::default();
        board.apply(&snapshot_of(&[session(1, NeedsYou, 0), session(2, Working, 0)]), T0);
        assert!(board.dismiss_selected().is_none());
        board.move_cursor(CursorMove::Down);
        assert!(board.dismiss_selected().is_none());
        assert_eq!(ids(&board).len(), 2);
    }

    #[test]
    fn jump_targets_selection() {
        let mut board = SessionBoard::default();
        assert!(board.jump_to_selected().is_none());
        board.apply(&snapshot_of(&[session(1, NeedsYou, 0), session(2, Working, 0)]), T0);
        board.move_cursor(CursorMove::Down);
        assert_eq!(board.jump_to_selected(), Some(Command::Jump(SessionId(2))));
    }

    #[test]
    fn time_in_state_counts_locally_and_freezes() {
        let mut board = SessionBoard::default();
        board.apply(&snapshot_of(&[session(1, Working, 10)]), T0);
        let row = board.rows()[0].clone();
        assert_eq!(board.time_in_state(&row, T0 + Millis(5_000)), Millis(15_000));

        board.freeze_time(Some(T0 + Millis(2_000)));
        assert_eq!(board.time_in_state(&row, T0 + Millis(60_000)), Millis(12_000));
    }
}
