//! Colours.

use desk_core::domain::{Agent, SessionState};

use crate::canvas::Rgb888;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    #[must_use]
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// 0xRRGGBB, the form the canvas takes.
    #[must_use]
    pub const fn hex(self) -> Rgb888 {
        (self.r as u32) << 16 | (self.g as u32) << 8 | self.b as u32
    }
}

/// `t` of the way from `from` to `to`, per channel, rounded.
#[must_use]
pub fn mix(from: Rgb, to: Rgb, t: f32) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    let channel = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * t + 0.5) as u8;
    Rgb { r: channel(from.r, to.r), g: channel(from.g, to.g), b: channel(from.b, to.b) }
}

// Dark, warm-neutral surfaces; Apple system colours (dark appearance) for state.
pub const BACKGROUND: Rgb = Rgb::new(11, 11, 13);
pub const SURFACE: Rgb = Rgb::new(36, 37, 42);
pub const LIFTED: Rgb = Rgb::new(52, 53, 60);
pub const HAIRLINE: Rgb = Rgb::new(40, 41, 46);
pub const TEXT: Rgb = Rgb::new(238, 238, 242);
pub const TEXT_DIM: Rgb = Rgb::new(150, 152, 160);
pub const TEXT_FAINT: Rgb = Rgb::new(92, 94, 102);

pub const NEEDS_YOU: Rgb = Rgb::new(255, 69, 58);
pub const DONE: Rgb = Rgb::new(255, 214, 10);
pub const WORKING: Rgb = Rgb::new(48, 209, 88);
pub const JUMP: Rgb = Rgb::new(10, 132, 255);

pub const CLAUDE: Rgb = Rgb::new(217, 119, 87);
pub const CODEX: Rgb = Rgb::new(214, 216, 226);

#[must_use]
pub const fn state_color(state: SessionState) -> Rgb {
    match state {
        SessionState::NeedsYou => NEEDS_YOU,
        SessionState::Done => DONE,
        SessionState::Working => WORKING,
    }
}

#[must_use]
pub const fn agent_color(agent: Agent) -> Rgb {
    match agent {
        Agent::Claude => CLAUDE,
        Agent::Codex => CODEX,
    }
}
