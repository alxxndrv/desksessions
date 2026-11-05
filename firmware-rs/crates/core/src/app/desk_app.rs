use super::backlight::{BacklightController, NightHours};
use super::button_gestures::{Button, ButtonGestures, Gesture, GestureKind};
use super::connection::{Connection, LinkEvent, Phase};
use super::wall_clock::{WallClock, WallTime};
use super::{DeviceSettings, MacLink};
use crate::domain::{Alert, CursorMove, CursorMoved, SessionBoard, Snapshot};
use crate::protocol::Frame;
use crate::scene::{BoardScene, StageScene};
use crate::time::{Instant, Millis};

/// Raw inputs for one tick.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Inputs {
    pub boot_down: bool,
    pub aux_down: bool,
}

/// The application: owns the board and its scene, turns gestures into board operations and
/// commands for the Mac. Runs on the UI task only.
#[derive(Clone, Debug, Default)]
pub struct DeskApp {
    backlight: BacklightController,
    wall_clock: WallClock,
    board: SessionBoard,
    scene: BoardScene,
    stage: StageScene,
    gestures: ButtonGestures,
    connection: Connection,
    phase: Phase,
    settings: DeviceSettings,
    last_interaction: Instant,
}

impl DeskApp {
    pub const CURSOR_IDLE_RESET: Millis = Millis(30_000);

    /// A fresh snapshot from the Mac.
    pub fn receive_snapshot(&mut self, snapshot: &Snapshot, now: Instant) -> Alert {
        let alert = self.board.apply(snapshot, now);
        self.scene.sync(&self.board, now);
        self.backlight.alert(alert, now);
        alert
    }

    /// A decoded frame from the Mac: its clock, quiet hours and snapshot.
    pub fn receive(&mut self, frame: &Frame, now: Instant) -> Alert {
        self.set_time(frame.seconds_of_day, now);
        self.set_night_hours(NightHours {
            start_hour: frame.night_start,
            end_hour: frame.night_end,
        });
        self.apply_settings(frame.settings);
        self.receive_snapshot(&frame.snapshot, now)
    }

    /// The BLE link changed state.
    pub fn link_changed(&mut self, event: LinkEvent, now: Instant) {
        self.connection.on_event(event, now);
    }

    /// The Mac's local time: `seconds_of_day` past midnight at `now`.
    pub fn set_time(&mut self, seconds_of_day: u32, now: Instant) {
        self.wall_clock.set(seconds_of_day, now);
    }

    pub fn set_night_hours(&mut self, hours: NightHours) {
        self.backlight.set_night_hours(hours);
    }

    pub fn apply_settings(&mut self, settings: DeviceSettings) {
        self.settings = settings;
        self.backlight.set_brightness(settings.day_brightness, settings.night_brightness);
    }

    #[must_use]
    pub fn settings(&self) -> &DeviceSettings {
        &self.settings
    }

    /// Advances one frame: buttons, idle behaviour, animation. Commands go out through `link`.
    /// Returns the backlight level for this frame, in [0, 1].
    pub fn tick(&mut self, inputs: Inputs, now: Instant, link: &mut impl MacLink) -> f32 {
        self.phase = self.connection.phase(now);
        if self.phase.away()
            && let Some(lost) = self.connection.lost_at()
        {
            self.board.freeze_time(Some(lost)); // the numbers stopped being true back then
        }

        // "Boot" means the up button wherever it ended up on the case.
        let (up, down) = if self.settings.swap_buttons {
            (inputs.aux_down, inputs.boot_down)
        } else {
            (inputs.boot_down, inputs.aux_down)
        };
        if let Some(gesture) = self.gestures.update(up, down, now) {
            self.last_interaction = now;
            self.on_gesture(gesture, now, link);
        }
        let hold = self.gestures.hold(now);
        self.scene.hold(hold.map(|h| h.progress), now);

        if self.board.cursor() != 0 && now - self.last_interaction >= Self::CURSOR_IDLE_RESET {
            // Nobody's touched it for a while: drift back to the most urgent row.
            let before = self.board.cursor();
            self.board.reset_cursor();
            self.scene.cursor_moved(
                &self.board,
                CursorMoved { moved: before != 0, wrapped: false },
                now,
            );
        }

        self.scene.update(now);
        self.stage.update(self.phase, self.board.is_empty(), self.board.rows().len(), now);
        let wall_time = self.wall_clock.now(now);
        self.backlight.level(self.phase, self.board.is_empty(), wall_time, now)
    }

    fn on_gesture(&mut self, gesture: Gesture, now: Instant, link: &mut impl MacLink) {
        match gesture.kind {
            GestureKind::Down => {
                let direction = if gesture.button == Button::Boot { -1.0 } else { 1.0 };
                self.scene.nudge(direction, now);
            }
            GestureKind::Press => {
                let direction =
                    if gesture.button == Button::Boot { CursorMove::Up } else { CursorMove::Down };
                self.move_cursor(direction, now);
            }
            GestureKind::LongPress => {
                let command = if gesture.button == Button::Aux {
                    self.board.dismiss_selected()
                } else {
                    self.board.jump_to_selected()
                };
                let Some(command) = command else {
                    self.scene.hold(None, now);
                    self.scene.refuse(now);
                    return;
                };
                self.scene.long_press_fired(now);
                link.send(command);
                self.scene.sync(&self.board, now);
            }
            GestureKind::Chord => link.forget_pairing(),
        }
    }

    fn move_cursor(&mut self, direction: CursorMove, now: Instant) {
        let moved = self.board.move_cursor(direction);
        if moved.moved {
            self.scene.cursor_moved(&self.board, moved, now);
        } else {
            self.scene.refuse(now);
        }
    }

    #[must_use]
    pub fn board(&self) -> &SessionBoard {
        &self.board
    }

    #[must_use]
    pub fn scene(&self) -> &BoardScene {
        &self.scene
    }

    #[must_use]
    pub fn gestures(&self) -> &ButtonGestures {
        &self.gestures
    }

    #[must_use]
    pub fn stage(&self) -> &StageScene {
        &self.stage
    }

    #[must_use]
    pub fn connection(&self) -> &Connection {
        &self.connection
    }

    #[must_use]
    pub fn phase(&self) -> Phase {
        self.phase
    }

    #[must_use]
    pub fn wall_time(&self, now: Instant) -> Option<WallTime> {
        self.wall_clock.now(now)
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::vec::Vec;

    use super::*;
    use crate::domain::{Command, Session, SessionId, SessionState};

    /// Records every command the app sends to the Mac.
    #[derive(Default)]
    struct RecordingLink {
        sent: Vec<Command>,
        forgot_pairing: usize,
    }

    impl MacLink for RecordingLink {
        fn send(&mut self, command: Command) {
            self.sent.push(command);
        }
        fn forget_pairing(&mut self) {
            self.forgot_pairing += 1;
        }
    }

    struct Fixture {
        now: Instant,
        link: RecordingLink,
        app: DeskApp,
    }

    impl Fixture {
        fn new() -> Self {
            let mut fixture = Self {
                now: Instant::from_millis(5_000),
                link: RecordingLink::default(),
                app: DeskApp::default(),
            };
            let mut snapshot = Snapshot::default();
            for (id, state) in
                [(1, SessionState::NeedsYou), (2, SessionState::Done), (3, SessionState::Working)]
            {
                let s = Session { id: SessionId(id), state, ..Session::default() };
                snapshot.sessions.push(s).unwrap();
            }
            fixture.app.receive_snapshot(&snapshot, fixture.now);
            fixture
        }

        fn hold(&mut self, boot: bool, aux: bool, ms: i64) {
            for _ in 0..ms {
                self.now += Millis(1);
                self.app.tick(Inputs { boot_down: boot, aux_down: aux }, self.now, &mut self.link);
            }
        }

        fn tap(&mut self, boot: bool, aux: bool) {
            self.hold(boot, aux, 80);
            self.hold(false, false, 80);
        }
    }

    #[test]
    fn taps_move_the_cursor() {
        let mut f = Fixture::new();
        f.tap(false, true);
        assert_eq!(f.app.board().cursor(), 1);
        f.tap(true, false);
        assert_eq!(f.app.board().cursor(), 0);
    }

    #[test]
    fn long_press_on_done_dismisses_and_tells_the_mac() {
        let mut f = Fixture::new();
        f.tap(false, true);
        f.hold(false, true, 700);
        f.hold(false, false, 50);
        assert_eq!(f.link.sent, [Command::Dismiss(SessionId(2))]);
        assert_eq!(f.app.board().rows().len(), 2);
    }

    #[test]
    fn long_press_boot_jumps() {
        let mut f = Fixture::new();
        f.hold(true, false, 700);
        assert_eq!(f.link.sent, [Command::Jump(SessionId(1))]);
    }

    #[test]
    fn dismissing_a_non_done_row_sends_nothing() {
        let mut f = Fixture::new();
        f.hold(false, true, 700);
        assert!(f.link.sent.is_empty());
        assert_eq!(f.app.board().rows().len(), 3);
    }

    #[test]
    fn cursor_drifts_home_after_thirty_seconds_idle() {
        let mut f = Fixture::new();
        f.tap(false, true);
        f.tap(false, true);
        f.hold(false, false, 29_000);
        assert_eq!(f.app.board().cursor(), 2);
        f.hold(false, false, 1_100);
        assert_eq!(f.app.board().cursor(), 0);
    }

    #[test]
    fn frames_set_the_clock_and_night_hours() {
        let mut f = Fixture::new();
        let frame = Frame {
            seconds_of_day: 9 * 3600 + 15 * 60,
            night_start: 21,
            night_end: 6,
            ..Frame::default()
        };
        f.app.receive(&frame, f.now);
        assert_eq!(f.app.wall_time(f.now), Some(WallTime { hour: 9, minute: 15 }));
        assert!(f.app.board().is_empty());
    }

    #[test]
    fn swapped_buttons_trade_places() {
        let mut f = Fixture::new();
        f.app.apply_settings(DeviceSettings { swap_buttons: true, ..DeviceSettings::default() });
        f.tap(true, false); // BOOT now moves down
        assert_eq!(f.app.board().cursor(), 1);
    }

    #[test]
    fn offline_freezes_timers_and_dims_after_grace() {
        let mut link = RecordingLink::default();
        let mut app = DeskApp::default();
        let mut now = Instant::from_millis(1_000);
        app.link_changed(LinkEvent::Connected, now);
        let mut snapshot = Snapshot::default();
        snapshot.sessions.push(Session { id: SessionId(1), ..Session::default() }).unwrap();
        app.receive_snapshot(&snapshot, now);

        app.link_changed(LinkEvent::Disconnected, now);
        let mut i = 0;
        while i < 12_000 {
            now += Millis(16);
            app.tick(Inputs::default(), now, &mut link);
            i += 16;
        }
        assert_eq!(app.phase(), Phase::Offline);
        assert!((app.stage().offline() - 1.0).abs() < 0.01);
        assert_eq!(app.board().time_in_state(&app.board().rows()[0], now), Millis::ZERO);
    }

    #[test]
    fn chord_forgets_pairing() {
        let mut link = RecordingLink::default();
        let mut app = DeskApp::default();
        let mut now = Instant::from_millis(1_000);
        for _ in 0..5_200 {
            now += Millis(1);
            app.tick(Inputs { boot_down: true, aux_down: true }, now, &mut link);
        }
        assert_eq!(link.forgot_pairing, 1);
    }
}
