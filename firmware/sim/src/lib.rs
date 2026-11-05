//! The simulator's engine: the firmware's app, renderer and a pretend Mac, stepped one frame at
//! a time. The window, the headless runner and the golden tests all drive it.

pub mod options;
pub mod recorder;

use std::fmt::Write as _;

use desk_core::app::{DeskApp, Inputs, LinkEvent};
use desk_core::domain::{Command, SessionState};
use desk_core::time::{Instant, Millis};
use desk_demo::Scenario;
use desk_ui::{Canvas, FrameMeter, Renderer, dim_for_backlight, expand565};

pub use options::{Action, Cue, Options};

pub const FPS: u32 = 60;
pub const WIDTH: usize = 320;
pub const HEIGHT: usize = 170;

/// Simulated monotonic time. Live, it follows the wall clock (optionally slowed). When
/// capturing, it advances exactly one frame per frame so recordings are smooth and repeatable.
#[derive(Debug)]
pub struct SimClock {
    last: std::time::Instant,
    elapsed: f64,
    speed: f64,
    fixed_step: bool,
}

impl SimClock {
    /// "Seconds ago" seeds stay positive.
    const START: f64 = 100_000.0;

    #[must_use]
    pub fn new(speed: f64, fixed_step: bool) -> Self {
        Self { last: std::time::Instant::now(), elapsed: 0.0, speed, fixed_step }
    }

    pub fn tick(&mut self) -> Instant {
        if self.fixed_step {
            self.elapsed += 1000.0 / f64::from(FPS) * self.speed;
        } else {
            let real = std::time::Instant::now();
            self.elapsed += (real - self.last).as_secs_f64() * 1000.0 * self.speed;
            self.last = real;
        }
        self.now()
    }

    #[must_use]
    pub fn now(&self) -> Instant {
        Instant::from_millis((Self::START + self.elapsed) as i64)
    }

    #[must_use]
    pub fn since_launch(&self) -> Millis {
        Millis(self.elapsed as i64)
    }

    pub fn toggle_slow_motion(&mut self) {
        self.speed = if self.speed < 1.0 { 1.0 } else { 0.25 };
    }
}

/// A pretend BLE link: connected, lost, or walking through pairing on a timer.
#[derive(Debug)]
pub struct SimLink {
    connected: bool,
    pairing_from: Option<Instant>,
    step: u8,
}

impl Default for SimLink {
    fn default() -> Self {
        Self { connected: true, pairing_from: None, step: 0 }
    }
}

impl SimLink {
    pub fn toggle(&mut self, app: &mut DeskApp, now: Instant) {
        self.connected = !self.connected;
        let event = if self.connected { LinkEvent::Connected } else { LinkEvent::Disconnected };
        app.link_changed(event, now);
    }

    /// Drops the link as if five minutes ago: straight to asleep.
    pub fn sleep(&mut self, app: &mut DeskApp, now: Instant) {
        self.connected = false;
        app.link_changed(LinkEvent::Disconnected, now - Millis(300_000));
    }

    pub fn pair(&mut self, app: &mut DeskApp, now: Instant) {
        self.connected = false;
        self.pairing_from = Some(now);
        app.link_changed(LinkEvent::Unpaired, now);
    }

    pub fn advance(&mut self, app: &mut DeskApp, now: Instant) {
        let Some(from) = self.pairing_from else { return };
        let t = now - from;
        if self.step == 0 && t >= Millis(2_500) {
            app.link_changed(LinkEvent::Passkey(482_913), now);
            self.step = 1;
        } else if self.step == 1 && t >= Millis(6_500) {
            app.link_changed(LinkEvent::Connected, now);
            self.connected = true;
            self.pairing_from = None;
            self.step = 0;
        }
    }
}

/// Keyboard state for one frame: the two buttons as held, everything else as fresh presses.
#[derive(Clone, Copy, Debug, Default)]
pub struct Keys {
    pub boot: bool,
    pub aux: bool,
    pub add: bool,
    pub remove: bool,
    pub escalate: bool,
    pub pause: bool,
    pub slow_motion: bool,
    pub link: bool,
    pub sleep: bool,
    pub pair: bool,
    pub empty: bool,
}

/// The local time of day, in seconds past midnight.
#[must_use]
pub fn local_seconds_of_day() -> u32 {
    use chrono::Timelike as _;
    chrono::Local::now().num_seconds_from_midnight()
}

/// The whole simulated clock.
pub struct Simulation {
    pub options: Options,
    clock: SimClock,
    scenario: Scenario,
    app: DeskApp,
    link: SimLink,
    renderer: Renderer,
    canvas: Canvas<Vec<u16>>,
    backlight: f32,
    next_cue: usize,
    /// With `--overlay`: frame timings, and when the last frame started.
    meter: Option<(FrameMeter, std::time::Instant)>,
}

impl Simulation {
    #[must_use]
    pub fn new(options: Options) -> Self {
        let clock = SimClock::new(options.speed, options.capturing());
        let options_overlay = options.overlay;
        let mut scenario = Scenario::new(clock.now());
        if !options.demo {
            scenario.toggle_auto();
        }
        let mut app = DeskApp::default();
        app.set_time(options.seconds_of_day.unwrap_or_else(local_seconds_of_day), clock.now());
        app.link_changed(LinkEvent::Connected, clock.now());
        Self {
            options,
            clock,
            scenario,
            app,
            link: SimLink::default(),
            renderer: Renderer::new(),
            canvas: Canvas::new(vec![0; WIDTH * HEIGHT], WIDTH as i32, HEIGHT as i32),
            backlight: 1.0,
            next_cue: 0,
            meter: options_overlay.then(|| (FrameMeter::new(), std::time::Instant::now())),
        }
    }

    /// Runs one frame: input, the pretend Mac, the app, and drawing. Returns the command the
    /// clock sent the Mac this frame, if any.
    pub fn step(&mut self, keys: &Keys) -> Option<Command> {
        let now = self.clock.tick();
        let t = self.clock.since_launch();

        if keys.add {
            self.scenario.add_session(now);
        }
        if keys.remove {
            self.scenario.remove_last();
        }
        if keys.escalate {
            self.scenario.escalate_first_working(now);
        }
        if keys.pause {
            self.scenario.toggle_auto();
        }
        if keys.slow_motion {
            self.clock.toggle_slow_motion();
        }
        if keys.link {
            self.link.toggle(&mut self.app, now);
        }
        if keys.sleep {
            self.link.sleep(&mut self.app, now);
        }
        if keys.pair || self.scenario.take_forget_request() {
            self.link.pair(&mut self.app, now);
        }
        if keys.empty {
            self.scenario.toggle_empty();
        }
        while let Some(cue) = self.options.cues.get(self.next_cue).filter(|c| c.at <= t) {
            match cue.action {
                Action::Add => self.scenario.add_session(now),
                Action::Remove => self.scenario.remove_last(),
                Action::Escalate => self.scenario.escalate_first_working(now),
                Action::Pause => self.scenario.toggle_auto(),
                Action::Link => self.link.toggle(&mut self.app, now),
                Action::Sleep => self.link.sleep(&mut self.app, now),
                Action::Pair => self.link.pair(&mut self.app, now),
                Action::Empty => self.scenario.toggle_empty(),
                Action::Up | Action::Down => {}
            }
            self.next_cue += 1;
        }

        self.link.advance(&mut self.app, now);
        if self.scenario.advance(now) {
            self.app.receive_snapshot(&self.scenario.snapshot(now), now);
        }
        let inputs = Inputs {
            boot_down: keys.boot || self.options.cue_holds(Action::Up, t),
            aux_down: keys.aux || self.options.cue_holds(Action::Down, t),
        };
        self.backlight = self.app.tick(inputs, now, &mut self.scenario);
        let heard = self.scenario.take_heard();
        if self.scenario.advance(now) {
            // a dismissal this tick
            self.app.receive_snapshot(&self.scenario.snapshot(now), now);
        }

        let drawing = std::time::Instant::now();
        self.renderer.draw(&self.app, now, &mut self.canvas);
        let draw_us = drawing.elapsed().as_micros() as u32;
        if self.options.backlight {
            for pixel in self.canvas.pixels_mut() {
                *pixel = dim_for_backlight(*pixel, self.backlight);
            }
        }
        if let Some((meter, last)) = &mut self.meter {
            // Captures run flat out, one simulated frame per frame: chart that, not the host.
            let frame_us = if self.options.capturing() {
                1_000_000 / FPS
            } else {
                drawing.duration_since(*last).as_micros() as u32
            };
            *last = drawing;
            meter.record(frame_us, draw_us);
            meter.draw(&mut self.canvas);
        }
        heard
    }

    #[must_use]
    pub fn app(&self) -> &DeskApp {
        &self.app
    }

    #[must_use]
    pub fn now(&self) -> Instant {
        self.clock.now()
    }

    #[must_use]
    pub fn since_launch(&self) -> Millis {
        self.clock.since_launch()
    }

    #[must_use]
    pub fn backlight(&self) -> f32 {
        self.backlight
    }

    #[must_use]
    pub fn canvas(&self) -> &Canvas<Vec<u16>> {
        &self.canvas
    }

    /// The frame as packed RGB, 8 bits per channel.
    #[must_use]
    pub fn rgb(&self) -> Vec<u8> {
        self.canvas
            .pixels()
            .iter()
            .flat_map(|&p| {
                let (r, g, b) = expand565(p);
                [r, g, b]
            })
            .collect()
    }

    /// The app's animated state this frame, one line, in the C++ reference dumper's layout.
    #[must_use]
    pub fn trace_line(&self) -> String {
        let app = &self.app;
        let scene = app.scene();
        let stage = app.stage();
        let mut out = String::new();
        let _ = write!(
            out,
            "t {} now {} phase {} cursor {} rows {} bl {}",
            self.since_launch().count(),
            self.now().since_boot.count(),
            app.phase() as u8,
            app.board().cursor(),
            app.board().rows().len(),
            self.backlight
        );
        let _ = write!(
            out,
            " hl {} hlo {} scroll {} nudge {} fill {}",
            scene.highlight_y(),
            scene.highlight_opacity(),
            scene.scroll(),
            scene.nudge_offset(),
            scene.hold_fill()
        );
        let _ = write!(
            out,
            " counts {} {} {}",
            scene.count(SessionState::NeedsYou),
            scene.count(SessionState::Done),
            scene.count(SessionState::Working)
        );
        let _ = write!(
            out,
            " stage {} {} {} {} {}",
            stage.quiet(),
            stage.offline(),
            stage.asleep(),
            stage.pairing(),
            stage.wake()
        );
        let _ = write!(out, " sel {}", scene.selected_id().map_or(0, |id| id.0));
        for slot in scene.slots() {
            let _ = write!(
                out,
                " | {} {} {} {} {} {} {} {} {} {} {}",
                slot.session.id.0,
                u8::from(slot.exiting),
                slot.session.state as u8,
                slot.previous_state as u8,
                slot.y.value(),
                slot.x.value(),
                slot.opacity.value(),
                slot.lift.value(),
                slot.state_blend.value(),
                slot.scale.value(),
                slot.glyph_scale.value()
            );
        }
        out
    }
}
