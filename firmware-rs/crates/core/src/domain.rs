//! What the clock knows: sessions, the sorted board with its cursor, and commands for the Mac.

mod session_board;

pub use session_board::{Alert, CursorMove, CursorMoved, SessionBoard, StateCounts};

use crate::text::Text;
use crate::time::Millis;

pub const MAX_SESSIONS: usize = 16;
/// UTF-8 bytes.
pub const MAX_NAME_BYTES: usize = 48;
/// UTF-8 bytes.
pub const MAX_DETAIL_BYTES: usize = 12;

/// Stable identifier the Mac assigns to a session for as long as it lives.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SessionId(pub u32);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Agent {
    #[default]
    Claude,
    Codex,
}

/// Ordered by urgency: earlier variants sort first on the board.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum SessionState {
    NeedsYou,
    Done,
    #[default]
    Working,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Session {
    pub id: SessionId,
    pub agent: Agent,
    pub state: SessionState,
    pub name: Text<MAX_NAME_BYTES>,
    /// Current tool, "perm", "?" …
    pub detail: Text<MAX_DETAIL_BYTES>,
    pub subagents: u8,
    /// Time spent in `state` when the snapshot was taken.
    pub in_state: Millis,
}

/// Everything the Mac currently wants shown, in any order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub sessions: heapless::Vec<Session, MAX_SESSIONS>,
}

/// A request the clock sends back to the Mac.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Dismiss(SessionId),
    Jump(SessionId),
}

impl Command {
    #[must_use]
    pub fn session(&self) -> SessionId {
        match *self {
            Self::Dismiss(id) | Self::Jump(id) => id,
        }
    }
}
