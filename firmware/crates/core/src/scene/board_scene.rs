use super::layout;
use crate::anim::{
    BOUNCY, Channel, Curve, MOOOK, Motion, SMOOTH, SNAPPY, SpringParams, spring_from,
};
use crate::domain::{CursorMoved, MAX_SESSIONS, Session, SessionBoard, SessionId, SessionState};
use crate::time::{Instant, Millis};

const REFLOW: Motion = Motion::springy(SMOOTH);
const REORDER: Motion = Motion::springy(SNAPPY);
const CURSOR: Motion = Motion::moook(&MOOOK);
const SCROLL: Motion = Motion::tween(Curve::EaseInOut, Millis(250));
const FADE_IN: Motion = Motion::tween(Curve::EaseOut, Millis(200));
const FADE_OUT: Motion = Motion::tween(Curve::EaseIn, MOOOK.duration());
const CROSSFADE: Motion = Motion::tween(Curve::CaDefault, Millis(250));
const COUNT: Motion = Motion::springy(SNAPPY);
const WIGGLE: SpringParams = spring_from(0.18, 0.55);

/// Slots outnumber sessions so rows can finish sliding out while new ones arrive.
pub const SLOTS: usize = MAX_SESSIONS + 4;

fn row_top(index: usize) -> f32 {
    index as f32 * layout::ROW_HEIGHT
}

/// One on-screen row and everything about it that moves. Slots outlive their sessions briefly
/// so a removed row can slide away, and a row that comes back mid-exit reverses instead of
/// popping.
#[derive(Clone, Debug)]
pub struct RowSlot {
    pub session: Session,
    pub previous_state: SessionState,
    pub exiting: bool,
    /// When this row entered its current state, on the local clock.
    pub state_since: Instant,

    /// Top edge in list coordinates.
    pub y: Channel,
    /// Horizontal offset: enter, exit, refusal wiggle.
    pub x: Channel,
    /// 0 → 1.
    pub opacity: Channel,
    /// 0 → 1 brightening while held or reordering.
    pub lift: Channel,
    /// 0 → 1 crossfade from `previous_state`'s colour.
    pub state_blend: Channel,
    /// Long-press pop.
    pub scale: Channel,
    /// DONE check pop.
    pub glyph_scale: Channel,
}

impl Default for RowSlot {
    fn default() -> Self {
        Self {
            session: Session::default(),
            previous_state: SessionState::Working,
            exiting: false,
            state_since: Instant::default(),
            y: Channel::default(),
            x: Channel::new(0.0),
            opacity: Channel::default(),
            lift: Channel::default(),
            state_blend: Channel::default(),
            scale: Channel::new(1.0),
            glyph_scale: Channel::new(1.0),
        }
    }
}

impl RowSlot {
    pub fn update(&mut self, now: Instant) {
        self.y.update(now);
        self.x.update(now);
        self.opacity.update(now);
        self.lift.update(now);
        self.state_blend.update(now);
        self.scale.update(now);
        self.glyph_scale.update(now);
    }
}

/// The animated picture of a [`SessionBoard`]: retained motion state, rebuilt from the board by
/// diffing on session id. Holds no drawing code; the renderer reads it every frame.
#[derive(Clone, Debug, Default)]
pub struct BoardScene {
    slots: heapless::Vec<RowSlot, SLOTS>,
    highlight: Channel,
    highlight_opacity: Channel,
    scroll: Channel,
    nudge: Channel,
    hold_fill: Channel,
    needs_you: Channel,
    done: Channel,
    working: Channel,
    visible_rows: usize,
    selected: Option<SessionId>,
    selected_since: Instant,
    holding: bool,
}

impl BoardScene {
    /// Reconciles rows with the board after a snapshot, a dismissal or a cursor reset.
    pub fn sync(&mut self, board: &SessionBoard, now: Instant) {
        let rows = board.rows();

        for slot in &mut self.slots {
            let still_there = rows.iter().any(|s| s.id == slot.session.id);
            if !still_there && !slot.exiting {
                slot.exiting = true;
                slot.x.animate_to(layout::EXIT_OFFSET, Motion::moook(&MOOOK), now);
                slot.opacity.animate_to(0.0, FADE_OUT, now);
            }
        }

        for (i, session) in rows.iter().enumerate() {
            let top = row_top(i);
            let Some(slot) = self.find_mut(session.id) else {
                self.enter(session, top, now);
                continue;
            };

            if slot.exiting {
                // Came back while sliding out: reverse from wherever it got to.
                slot.exiting = false;
                slot.x.animate_to(0.0, REORDER, now);
                slot.opacity.animate_to(1.0, FADE_IN, now);
            }

            let state_changed = slot.session.state != session.state;
            if state_changed {
                slot.previous_state = slot.session.state;
                slot.state_since = now;
                slot.state_blend.snap_to(0.0);
                slot.state_blend.animate_to(1.0, CROSSFADE, now);
                if session.state == SessionState::Done {
                    slot.glyph_scale.snap_to(0.0);
                    slot.glyph_scale.animate_to(1.0, Motion::springy(BOUNCY), now);
                    slot.glyph_scale.kick(6.0, now);
                }
            }
            slot.session = session.clone();

            if slot.y.target() != top {
                if state_changed {
                    slot.y.animate_to(top, REORDER, now);
                    slot.lift.animate_to(1.0, Motion::tween(Curve::EaseOut, Millis(120)), now);
                    slot.lift.then(0.0, REFLOW);
                } else {
                    slot.y.animate_to(top, REFLOW, now);
                }
            }
        }

        self.visible_rows = rows.len();
        let counts = board.counts();
        self.needs_you.animate_to(f32::from(counts.needs_you), COUNT, now);
        self.done.animate_to(f32::from(counts.done), COUNT, now);
        self.working.animate_to(f32::from(counts.working), COUNT, now);
        self.follow(board, now);
    }

    /// The cursor moved on `board` (already updated).
    pub fn cursor_moved(&mut self, board: &SessionBoard, moved: CursorMoved, now: Instant) {
        if !moved.moved {
            return;
        }
        let target = row_top(board.cursor());
        if moved.wrapped {
            // Stretch past the edge it is leaving, then jump to the other end.
            let from_bottom = target == 0.0;
            let stretch = if from_bottom { layout::WRAP_STRETCH } else { -layout::WRAP_STRETCH };
            let edge = self.highlight.target() + stretch;
            self.highlight.animate_to(edge, Motion::tween(Curve::CaEaseOut, Millis(66)), now);
            self.highlight.then(target, CURSOR);
        } else {
            self.highlight.animate_to(target, CURSOR, now);
        }
        self.selected = board.selected().map(|s| s.id);
        self.selected_since = now;
        self.scroll_to_reveal(target, now);
    }

    /// Press feedback: the list leans 5 px toward the button (-1 = up/BOOT, +1 = down/Aux).
    pub fn nudge(&mut self, direction: f32, now: Instant) {
        self.nudge.animate_to(
            direction * layout::NUDGE,
            Motion::tween(Curve::EaseOut, Millis(48)),
            now,
        );
        self.nudge.then(0.0, Motion::tween(Curve::EaseOut, Millis(144)));
    }

    /// A long press is filling (`progress` in [0, 1]) or was let go early (`None`).
    pub fn hold(&mut self, progress: Option<f32>, now: Instant) {
        if let Some(progress) = progress {
            self.holding = true;
            let shown =
                ((progress - layout::HOLD_REVEAL) / (1.0 - layout::HOLD_REVEAL)).clamp(0.0, 1.0);
            self.hold_fill.snap_to(shown);
            if let Some(slot) = self.selected_slot_mut() {
                slot.lift.snap_to(shown * layout::HOLD_LIFT);
            }
            return;
        }
        if self.holding {
            // Let go early: everything drains back.
            self.holding = false;
            self.hold_fill.animate_to(0.0, Motion::springy(SNAPPY), now);
            if let Some(slot) = self.selected_slot_mut() {
                slot.lift.animate_to(0.0, REFLOW, now);
            }
        }
    }

    /// The long press completed: the selected row pops.
    pub fn long_press_fired(&mut self, now: Instant) {
        self.holding = false;
        self.hold_fill.snap_to(1.0);
        self.hold_fill.animate_to(0.0, Motion::tween(Curve::EaseOut, Millis(250)), now);
        if let Some(slot) = self.selected_slot_mut() {
            slot.scale.kick(1.2, now);
            slot.lift.animate_to(0.0, REFLOW, now);
        }
    }

    /// The selected row can't do what was asked: it wiggles.
    pub fn refuse(&mut self, now: Instant) {
        if let Some(slot) = self.selected_slot_mut() {
            slot.x.kick_with(260.0, now, WIGGLE);
        }
    }

    pub fn update(&mut self, now: Instant) {
        for slot in &mut self.slots {
            slot.update(now);
        }
        self.slots.retain(|s| !(s.exiting && !s.x.animating()));
        self.highlight.update(now);
        self.highlight_opacity.update(now);
        self.scroll.update(now);
        self.nudge.update(now);
        self.hold_fill.update(now);
        self.needs_you.update(now);
        self.done.update(now);
        self.working.update(now);
    }

    #[must_use]
    pub fn slots(&self) -> &[RowSlot] {
        &self.slots
    }

    #[must_use]
    pub fn highlight_y(&self) -> f32 {
        self.highlight.value()
    }

    #[must_use]
    pub fn highlight_opacity(&self) -> f32 {
        self.highlight_opacity.value()
    }

    #[must_use]
    pub fn scroll(&self) -> f32 {
        self.scroll.value()
    }

    #[must_use]
    pub fn nudge_offset(&self) -> f32 {
        self.nudge.value()
    }

    #[must_use]
    pub fn hold_fill(&self) -> f32 {
        self.hold_fill.value()
    }

    #[must_use]
    pub fn selected_id(&self) -> Option<SessionId> {
        self.selected
    }

    #[must_use]
    pub fn selected_since(&self) -> Instant {
        self.selected_since
    }

    #[must_use]
    pub fn count(&self, state: SessionState) -> f32 {
        match state {
            SessionState::NeedsYou => self.needs_you.value(),
            SessionState::Done => self.done.value(),
            SessionState::Working => self.working.value(),
        }
    }

    #[must_use]
    pub fn content_height(&self) -> f32 {
        row_top(self.visible_rows)
    }

    fn find_mut(&mut self, id: SessionId) -> Option<&mut RowSlot> {
        self.slots.iter_mut().find(|s| s.session.id == id)
    }

    fn selected_slot_mut(&mut self) -> Option<&mut RowSlot> {
        let id = self.selected?;
        self.find_mut(id)
    }

    fn enter(&mut self, session: &Session, y: f32, now: Instant) {
        if self.slots.is_full() {
            // Out of slots only while many rows are still sliding out: drop the oldest leaver.
            let Some(leaver) = self.slots.iter().position(|s| s.exiting) else {
                return;
            };
            self.slots.remove(leaver);
        }
        let mut slot = RowSlot {
            session: session.clone(),
            previous_state: session.state,
            state_since: now,
            ..RowSlot::default()
        };
        slot.y.snap_to(y);
        slot.x.snap_to(layout::ENTER_OFFSET);
        slot.x.animate_to(0.0, REORDER, now);
        slot.opacity.snap_to(0.0);
        slot.opacity.animate_to(1.0, FADE_IN, now);
        slot.state_blend.snap_to(1.0);
        // Cannot fail: room was made above.
        let _ = self.slots.push(slot);
    }

    fn follow(&mut self, board: &SessionBoard, now: Instant) {
        let selected = board.selected();
        self.highlight_opacity.animate_to(if selected.is_some() { 1.0 } else { 0.0 }, FADE_IN, now);
        let Some(selected) = selected else {
            self.selected = None;
            return;
        };
        if self.selected != Some(selected.id) {
            self.selected = Some(selected.id);
            self.selected_since = now;
        }
        let target = row_top(board.cursor());
        if self.highlight.target() != target {
            // The selected row moved under the cursor: travel with it.
            self.highlight.animate_to(target, REORDER, now);
        }
        self.scroll_to_reveal(target, now);
    }

    fn scroll_to_reveal(&mut self, top: f32, now: Instant) {
        let max_scroll = (self.content_height() - layout::VIEWPORT_HEIGHT).max(0.0);
        let mut target = self.scroll.target();
        if top < target {
            target = top;
        } else if top + layout::ROW_HEIGHT > target + layout::VIEWPORT_HEIGHT {
            target = top + layout::ROW_HEIGHT - layout::VIEWPORT_HEIGHT;
        }
        target = target.clamp(0.0, max_scroll);
        if target != self.scroll.target() {
            self.scroll.animate_to(target, SCROLL, now);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assert_float_eq;
    use crate::domain::{CursorMove, Snapshot};

    fn session(id: u32, state: SessionState) -> Session {
        Session { id: SessionId(id), state, ..Session::default() }
    }

    fn snapshot_of(sessions: &[Session]) -> Snapshot {
        Snapshot { sessions: sessions.iter().cloned().collect() }
    }

    struct Fixture {
        now: Instant,
        board: SessionBoard,
        scene: BoardScene,
    }

    impl Fixture {
        fn new() -> Self {
            Self {
                now: Instant::from_millis(10_000),
                board: SessionBoard::default(),
                scene: BoardScene::default(),
            }
        }

        fn apply(&mut self, snapshot: &Snapshot) {
            self.board.apply(snapshot, self.now);
            self.scene.sync(&self.board, self.now);
        }

        fn run(&mut self, ms: i64) {
            let mut t = 0;
            while t < ms {
                self.now += Millis(16);
                self.scene.update(self.now);
                t += 16;
            }
        }

        fn slot(&self, id: u32) -> Option<&RowSlot> {
            self.scene.slots().iter().find(|s| s.session.id.0 == id)
        }

        fn move_cursor(&mut self, direction: CursorMove) {
            let moved = self.board.move_cursor(direction);
            self.scene.cursor_moved(&self.board, moved, self.now);
        }
    }

    use SessionState::{Done, NeedsYou, Working};

    #[test]
    fn new_rows_slide_in_from_the_right_and_settle() {
        let mut f = Fixture::new();
        f.apply(&snapshot_of(&[session(1, Working)]));
        assert_float_eq!(f.slot(1).unwrap().x.value(), layout::ENTER_OFFSET);
        assert_float_eq!(f.slot(1).unwrap().opacity.value(), 0.0);
        f.run(1_500);
        assert!(f.slot(1).unwrap().x.value().abs() < 0.05);
        assert_float_eq!(f.slot(1).unwrap().opacity.value(), 1.0);
    }

    #[test]
    fn removed_rows_slide_out_then_free_their_slot() {
        let mut f = Fixture::new();
        f.apply(&snapshot_of(&[session(1, Done), session(2, Working)]));
        f.run(1_500);
        f.apply(&snapshot_of(&[session(2, Working)]));
        assert!(f.slot(1).unwrap().exiting);
        f.run(100);
        assert!(f.slot(1).unwrap().x.value() < 0.0);
        f.run(400);
        assert!(f.slot(1).is_none());
        // Row 2 closed the gap.
        f.run(1_000);
        assert!(f.slot(2).unwrap().y.value().abs() < 0.05);
    }

    #[test]
    fn row_returning_mid_exit_reverses_instead_of_popping() {
        let mut f = Fixture::new();
        f.apply(&snapshot_of(&[session(1, Working)]));
        f.run(1_500);
        f.apply(&Snapshot::default());
        f.run(80);
        let midway = f.slot(1).unwrap().x.value();
        assert!(midway < 0.0);

        f.apply(&snapshot_of(&[session(1, Working)]));
        assert!(!f.slot(1).unwrap().exiting);
        assert_float_eq!(f.slot(1).unwrap().x.value(), midway);
        f.run(1_500);
        assert!(f.slot(1).unwrap().x.value().abs() < 0.05);
    }

    #[test]
    fn reordered_row_travels_with_the_highlight() {
        let mut f = Fixture::new();
        f.apply(&snapshot_of(&[session(1, Working), session(2, Working)]));
        f.run(1_500);
        f.board.move_cursor(CursorMove::Down);
        f.scene.cursor_moved(&f.board, CursorMoved { moved: true, wrapped: false }, f.now);
        f.run(500);
        assert_float_eq!(f.scene.highlight_y(), layout::ROW_HEIGHT);

        // Session 2 needs you: it moves to the top and the highlight goes with it.
        f.apply(&snapshot_of(&[session(1, Working), session(2, NeedsYou)]));
        f.run(1_500);
        assert!(f.slot(2).unwrap().y.value().abs() < 0.05);
        assert!(f.scene.highlight_y().abs() < 0.05);
        assert_eq!(f.scene.selected_id(), Some(SessionId(2)));
    }

    #[test]
    fn scrolls_to_reveal_the_cursor_and_wraps_back() {
        let mut f = Fixture::new();
        let many: heapless::Vec<Session, MAX_SESSIONS> =
            (1..=8).map(|id| session(id, Working)).collect();
        f.apply(&Snapshot { sessions: many });
        for _ in 0..6 {
            f.move_cursor(CursorMove::Down);
            f.run(48);
        }
        f.run(500);
        assert_float_eq!(f.scene.scroll(), 7.0 * layout::ROW_HEIGHT - layout::VIEWPORT_HEIGHT);

        f.move_cursor(CursorMove::Down); // row 8, bottom
        f.run(300);
        f.move_cursor(CursorMove::Down); // wraps to top
        f.run(30);
        assert!(f.scene.highlight_y() > 7.0 * layout::ROW_HEIGHT); // stretched past the bottom
        f.run(600);
        assert_float_eq!(f.scene.highlight_y(), 0.0);
        assert_float_eq!(f.scene.scroll(), 0.0);
    }

    #[test]
    fn hold_fill_drains_when_released_early() {
        let mut f = Fixture::new();
        f.apply(&snapshot_of(&[session(1, Done)]));
        f.scene.hold(Some(0.1), f.now);
        assert_float_eq!(f.scene.hold_fill(), 0.0); // a tap shows nothing
        f.scene.hold(Some(0.625), f.now);
        assert_float_eq!(f.scene.hold_fill(), 0.5);
        f.scene.hold(None, f.now);
        f.run(1_000);
        assert!(f.scene.hold_fill().abs() < 0.01);
    }

    #[test]
    fn counts_roll_to_new_values() {
        let mut f = Fixture::new();
        f.apply(&snapshot_of(&[session(1, NeedsYou), session(2, NeedsYou)]));
        f.run(1_500);
        assert!((f.scene.count(NeedsYou) - 2.0).abs() < 0.01);
        assert!(f.scene.count(Done).abs() < 0.01);
    }
}
