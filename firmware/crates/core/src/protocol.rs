//! The BLE wire format, as specified in `protocol/PROTOCOL.md`.

use crate::app::DeviceSettings;
use crate::domain::{
    Agent, Command, MAX_DETAIL_BYTES, MAX_NAME_BYTES, MAX_SESSIONS, Session, SessionId,
    SessionState, Snapshot,
};
use crate::text::{Text, complete_utf8_prefix};
use crate::time::Millis;

pub const VERSION: u8 = 1;
pub const CHUNK_HEADER_BYTES: usize = 3;
pub const MAX_CHUNKS: usize = 8;
pub const FRAME_HEADER_BYTES: usize = 11;
pub const MAX_SESSION_BYTES: usize = 4 + 1 + 1 + 1 + 4 + 1 + MAX_NAME_BYTES + 1 + MAX_DETAIL_BYTES;
pub const MAX_FRAME_BYTES: usize = FRAME_HEADER_BYTES + MAX_SESSIONS * MAX_SESSION_BYTES;
pub const COMMAND_BYTES: usize = 5;

/// Bits of the frame's `flags` byte.
pub const FLAG_FLIP_SCREEN: u8 = 1 << 0;
pub const FLAG_SWAP_BUTTONS: u8 = 1 << 1;

/// Everything one snapshot write carries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub snapshot: Snapshot,
    pub seconds_of_day: u32,
    pub night_start: u8,
    pub night_end: u8,
    pub settings: DeviceSettings,
}

impl Default for Frame {
    fn default() -> Self {
        Self {
            snapshot: Snapshot::default(),
            seconds_of_day: 0,
            night_start: 23,
            night_end: 8,
            settings: DeviceSettings::default(),
        }
    }
}

/// Bounds-checked little-endian reader.
struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn u8(&mut self) -> Option<u8> {
        let v = *self.bytes.get(self.pos)?;
        self.pos += 1;
        Some(v)
    }

    fn u32(&mut self) -> Option<u32> {
        let raw = self.bytes.get(self.pos..self.pos + 4)?;
        self.pos += 4;
        Some(u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]))
    }

    fn text<const N: usize>(&mut self) -> Option<Text<N>> {
        let length = usize::from(self.u8()?);
        if length > N {
            return None;
        }
        let raw: &'a [u8] = self.bytes.get(self.pos..self.pos + length)?;
        self.pos += length;
        Some(Text::from_bytes(raw))
    }

    fn done(&self) -> bool {
        self.pos == self.bytes.len()
    }
}

/// Little-endian writer that stops writing (and reports failure) when out of room.
struct Writer<'a> {
    out: &'a mut [u8],
    pos: usize,
    overflow: bool,
}

impl Writer<'_> {
    fn u8(&mut self, v: u8) {
        match self.out.get_mut(self.pos) {
            Some(slot) => *slot = v,
            None => self.overflow = true,
        }
        self.pos += 1;
    }

    fn u32(&mut self, v: u32) {
        for b in v.to_le_bytes() {
            self.u8(b);
        }
    }

    fn text(&mut self, text: &[u8], limit: usize) {
        let n = complete_utf8_prefix(text, text.len().min(limit));
        self.u8(n as u8);
        for &b in &text[..n] {
            self.u8(b);
        }
    }

    fn written(&self) -> usize {
        if self.overflow { 0 } else { self.pos }
    }
}

fn agent_from(raw: u8) -> Option<Agent> {
    match raw {
        0 => Some(Agent::Claude),
        1 => Some(Agent::Codex),
        _ => None,
    }
}

fn state_from(raw: u8) -> Option<SessionState> {
    match raw {
        0 => Some(SessionState::NeedsYou),
        1 => Some(SessionState::Done),
        2 => Some(SessionState::Working),
        _ => None,
    }
}

const fn agent_byte(agent: Agent) -> u8 {
    match agent {
        Agent::Claude => 0,
        Agent::Codex => 1,
    }
}

const fn state_byte(state: SessionState) -> u8 {
    match state {
        SessionState::NeedsYou => 0,
        SessionState::Done => 1,
        SessionState::Working => 2,
    }
}

/// Parses a reassembled frame. Rejects unknown versions, bad enums and overruns.
#[must_use]
pub fn decode_frame(bytes: &[u8]) -> Option<Frame> {
    let mut r = Reader { bytes, pos: 0 };
    if r.u8()? != VERSION {
        return None;
    }
    let flags = r.u8()?;
    let seconds_of_day = r.u32()?;
    let night_start = r.u8()?;
    let night_end = r.u8()?;
    let day_brightness = r.u8()?;
    let night_brightness = r.u8()?;
    let count = r.u8()?;
    if usize::from(count) > MAX_SESSIONS
        || night_start > 23
        || night_end > 23
        || day_brightness > 100
        || night_brightness > 100
    {
        return None;
    }
    let mut frame = Frame {
        snapshot: Snapshot::default(),
        seconds_of_day,
        night_start,
        night_end,
        settings: DeviceSettings {
            flip_screen: flags & FLAG_FLIP_SCREEN != 0,
            swap_buttons: flags & FLAG_SWAP_BUTTONS != 0,
            day_brightness,
            night_brightness,
        },
    };
    for _ in 0..count {
        let id = r.u32()?;
        let agent = r.u8()?;
        let state = r.u8()?;
        let subagents = r.u8()?;
        let in_state = r.u32()?;
        let name = r.text::<MAX_NAME_BYTES>()?;
        let detail = r.text::<MAX_DETAIL_BYTES>()?;
        let session = Session {
            id: SessionId(id),
            agent: agent_from(agent)?,
            state: state_from(state)?,
            name,
            detail,
            subagents,
            in_state: Millis(i64::from(in_state) * 1000),
        };
        // Cannot fail: `count` was checked against the capacity.
        let _ = frame.snapshot.sessions.push(session);
    }
    if !r.done() {
        return None;
    }
    Some(frame)
}

/// Serialises `frame` into `out`, truncating strings on UTF-8 boundaries. Returns bytes written,
/// or 0 when `out` is too small.
#[must_use]
pub fn encode_frame(frame: &Frame, out: &mut [u8]) -> usize {
    let mut w = Writer { out, pos: 0, overflow: false };
    let settings = &frame.settings;
    w.u8(VERSION);
    w.u8(if settings.flip_screen { FLAG_FLIP_SCREEN } else { 0 }
        | if settings.swap_buttons { FLAG_SWAP_BUTTONS } else { 0 });
    w.u32(frame.seconds_of_day);
    w.u8(frame.night_start);
    w.u8(frame.night_end);
    w.u8(settings.day_brightness);
    w.u8(settings.night_brightness);
    w.u8(frame.snapshot.sessions.len() as u8);
    for s in &frame.snapshot.sessions {
        w.u32(s.id.0);
        w.u8(agent_byte(s.agent));
        w.u8(state_byte(s.state));
        w.u8(s.subagents);
        w.u32((s.in_state.count() / 1000).max(0) as u32);
        w.text(s.name.as_bytes(), MAX_NAME_BYTES);
        w.text(s.detail.as_bytes(), MAX_DETAIL_BYTES);
    }
    w.written()
}

#[must_use]
pub fn encode_command(command: &Command) -> [u8; COMMAND_BYTES] {
    let kind = match command {
        Command::Dismiss(_) => 0,
        Command::Jump(_) => 1,
    };
    let id = command.session().0.to_le_bytes();
    [kind, id[0], id[1], id[2], id[3]]
}

#[must_use]
pub fn decode_command(bytes: &[u8]) -> Option<Command> {
    let mut r = Reader { bytes, pos: 0 };
    let kind = r.u8()?;
    if kind > 1 {
        return None;
    }
    let id = SessionId(r.u32()?);
    if !r.done() {
        return None;
    }
    Some(if kind == 0 { Command::Dismiss(id) } else { Command::Jump(id) })
}

/// Collects chunks back into a frame.
#[derive(Clone, Debug, Default)]
pub struct Reassembler {
    buffer: heapless::Vec<u8, MAX_FRAME_BYTES>,
    frame: Option<u8>,
    expected: u8,
}

impl Reassembler {
    /// Feeds one chunk. Returns the whole frame once its last chunk arrives; the view stays
    /// valid until the next push.
    pub fn push(&mut self, chunk: &[u8]) -> Option<&[u8]> {
        if chunk.len() < CHUNK_HEADER_BYTES {
            return None;
        }
        let (frame, index, count) = (chunk[0], chunk[1], chunk[2]);
        if count == 0 || usize::from(count) > MAX_CHUNKS || index >= count {
            return None;
        }
        if index == 0 || self.frame != Some(frame) {
            // A new frame starts; anything half-received is abandoned.
            self.buffer.clear();
            self.frame = Some(frame);
            self.expected = 0;
            if index != 0 {
                self.frame = None;
                return None;
            }
        }
        if index != self.expected {
            self.frame = None;
            return None;
        }
        let payload = &chunk[CHUNK_HEADER_BYTES..];
        if self.buffer.extend_from_slice(payload).is_err() {
            self.frame = None;
            return None;
        }
        self.expected += 1;
        if self.expected == count {
            self.frame = None;
            return Some(&self.buffer);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::vec::Vec;

    use super::*;

    fn sample() -> Frame {
        let mut frame = Frame {
            seconds_of_day: 13 * 3600 + 7 * 60,
            night_start: 22,
            night_end: 7,
            ..Frame::default()
        };
        frame.settings.swap_buttons = true;
        frame.settings.day_brightness = 55;
        let a = Session {
            id: SessionId(0xDEAD_BEEF),
            agent: Agent::Codex,
            state: SessionState::NeedsYou,
            subagents: 3,
            in_state: Millis(42_000),
            name: "Миграция базы".into(),
            detail: "perm".into(),
        };
        let b = Session { id: SessionId(7), name: "desksessions-d9".into(), ..Session::default() };
        frame.snapshot.sessions.push(a).unwrap();
        frame.snapshot.sessions.push(b).unwrap();
        frame
    }

    fn encoded(frame: &Frame) -> Vec<u8> {
        let mut buffer = [0u8; MAX_FRAME_BYTES];
        let n = encode_frame(frame, &mut buffer);
        buffer[..n].to_vec()
    }

    fn chunks(frame: &[u8], seq: u8, size: usize) -> Vec<Vec<u8>> {
        let payload = size - CHUNK_HEADER_BYTES;
        let count = frame.len().div_ceil(payload) as u8;
        (0..count)
            .map(|i| {
                let mut c = std::vec![seq, i, count];
                let start = usize::from(i) * payload;
                c.extend_from_slice(&frame[start..(start + payload).min(frame.len())]);
                c
            })
            .collect()
    }

    #[test]
    fn frame_round_trips() {
        let bytes = encoded(&sample());
        assert!(!bytes.is_empty());
        let decoded = decode_frame(&bytes).unwrap();
        assert_eq!(decoded.seconds_of_day, 13 * 3600 + 7 * 60);
        assert_eq!(decoded.night_start, 22);
        assert!(!decoded.settings.flip_screen);
        assert!(decoded.settings.swap_buttons);
        assert_eq!(decoded.settings.day_brightness, 55);
        assert_eq!(decoded.snapshot.sessions.len(), 2);
        let a = &decoded.snapshot.sessions[0];
        assert_eq!(a.id.0, 0xDEAD_BEEF);
        assert_eq!(a.agent, Agent::Codex);
        assert_eq!(a.state, SessionState::NeedsYou);
        assert_eq!(a.subagents, 3);
        assert_eq!(a.in_state, Millis(42_000));
        assert_eq!(a.name, "Миграция базы");
        assert_eq!(a.detail, "perm");
    }

    #[test]
    fn rejects_truncated_and_unknown_frames() {
        let mut bytes = encoded(&sample());
        assert!(decode_frame(&bytes[..bytes.len() - 1]).is_none());
        bytes[0] = 2;
        assert!(decode_frame(&bytes).is_none());
    }

    #[test]
    fn rejects_bad_enums() {
        let mut bytes = encoded(&sample());
        bytes[FRAME_HEADER_BYTES + 5] = 9; // first session's state
        assert!(decode_frame(&bytes).is_none());
    }

    #[test]
    fn reassembles_chunks_and_drops_interrupted_frames() {
        let bytes = encoded(&sample());
        let first = chunks(&bytes, 4, 20);
        assert!(first.len() > 2);

        let mut reassembler = Reassembler::default();
        // A new frame interrupts this one halfway.
        reassembler.push(&first[0]);
        let mut done = None;
        for c in chunks(&bytes, 5, 20) {
            done = reassembler.push(&c).map(<[u8]>::to_vec);
        }
        assert!(decode_frame(&done.unwrap()).is_some());

        // Out-of-order chunks are rejected.
        assert!(reassembler.push(&first[1]).is_none());
    }

    #[test]
    fn commands_round_trip() {
        let command = Command::Jump(SessionId(0x0102_0304));
        let bytes = encode_command(&command);
        assert_eq!(bytes[0], 1);
        assert_eq!(bytes[1], 0x04);
        assert_eq!(decode_command(&bytes), Some(command));
    }

    #[test]
    fn truncates_names_on_utf8_boundaries() {
        let mut frame = Frame::default();
        // 47 ASCII bytes then a two-byte letter: the name keeps 48 bytes, 47 + half a letter.
        let mut s = Session::default();
        s.name.push_str(&"a".repeat(47));
        s.name.push_str("ж");
        assert_eq!(s.name.len(), 48);
        frame.snapshot.sessions.push(s).unwrap();
        let decoded = decode_frame(&encoded(&frame)).unwrap();
        assert_eq!(decoded.snapshot.sessions[0].name.len(), 47);
    }

    // A full frame fits the chunk budget at the smallest MTU the Mac negotiates.
    const _: () = assert!(MAX_FRAME_BYTES <= MAX_CHUNKS * (182 - CHUNK_HEADER_BYTES));
}
