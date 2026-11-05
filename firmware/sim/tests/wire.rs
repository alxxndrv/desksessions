//! The wire format against vectors captured from the C++ firmware (`golden/fixtures/wire.txt`)
//! and the frame the Mac app's own tests encode (`protocol/golden/frame-v1.hex`).

use std::path::PathBuf;

use desk_core::domain::{Agent, Command, MAX_SESSIONS, Session, SessionId, SessionState};
use desk_core::protocol::{
    Frame, MAX_FRAME_BYTES, Reassembler, decode_command, decode_frame, encode_command, encode_frame,
};
use desk_core::time::Millis;

/// The workspace root.
fn workspace() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn hex(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return "-".into();
    }
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(text: &str) -> Vec<u8> {
    if text == "-" {
        return Vec::new();
    }
    (0..text.len() / 2).map(|i| u8::from_str_radix(&text[2 * i..2 * i + 2], 16).unwrap()).collect()
}

fn session(
    id: u32,
    agent: Agent,
    state: SessionState,
    subagents: u8,
    seconds: i64,
    name: &str,
    detail: &str,
) -> Session {
    Session {
        id: SessionId(id),
        agent,
        state,
        subagents,
        in_state: Millis(seconds * 1000),
        name: name.into(),
        detail: detail.into(),
    }
}

fn frames() -> Vec<(&'static str, Frame, usize)> {
    let mut sample = Frame {
        seconds_of_day: 13 * 3600 + 7 * 60,
        night_start: 22,
        night_end: 7,
        ..Frame::default()
    };
    sample.settings.swap_buttons = true;
    sample.settings.day_brightness = 55;
    sample
        .snapshot
        .sessions
        .push(session(
            0xDEAD_BEEF,
            Agent::Codex,
            SessionState::NeedsYou,
            3,
            42,
            "Миграция базы",
            "perm",
        ))
        .unwrap();
    sample
        .snapshot
        .sessions
        .push(session(7, Agent::Claude, SessionState::Working, 0, 0, "desksessions-d9", ""))
        .unwrap();

    let mut flags =
        Frame { seconds_of_day: 86_399, night_start: 0, night_end: 23, ..Frame::default() };
    flags.settings.flip_screen = true;
    flags.settings.swap_buttons = true;
    flags.settings.day_brightness = 100;
    flags.settings.night_brightness = 0;

    let mut truncated = Frame::default();
    let mut cut = Session::default();
    cut.name.push_str(&"a".repeat(47));
    cut.name.push_str("ж");
    cut.detail = "Подтверждение".into();
    truncated.snapshot.sessions.push(cut).unwrap();

    let mut full = Frame::default();
    for i in 0..MAX_SESSIONS as u32 {
        let state =
            [SessionState::NeedsYou, SessionState::Done, SessionState::Working][i as usize % 3];
        full.snapshot
            .sessions
            .push(session(
                1000 + i,
                if i % 2 == 1 { Agent::Codex } else { Agent::Claude },
                state,
                i as u8,
                3600 * i64::from(i) + 59,
                "A very long session name that certainly overflows its budget",
                "WebFetch12345",
            ))
            .unwrap();
    }

    vec![
        ("sample", sample.clone(), MAX_FRAME_BYTES),
        ("sample-short-buffer", sample, 20),
        ("empty", Frame::default(), MAX_FRAME_BYTES),
        ("flags", flags, MAX_FRAME_BYTES),
        ("utf8-truncation", truncated, MAX_FRAME_BYTES),
        ("full", full.clone(), MAX_FRAME_BYTES),
        ("full-one-short", full, MAX_FRAME_BYTES - 1),
    ]
}

fn encode(frame: &Frame, capacity: usize) -> Vec<u8> {
    let mut out = vec![0; capacity];
    let n = encode_frame(frame, &mut out);
    out.truncate(n);
    out
}

#[test]
fn matches_the_c_firmware_byte_for_byte() {
    let text =
        std::fs::read_to_string(workspace().join("golden/fixtures/wire.txt")).expect("wire.txt");
    let frames = frames();
    let mut commands = Vec::new();
    for id in [0u32, 1, 0x0102_0304, 0xFFFF_FFFF] {
        commands.push(Command::Dismiss(SessionId(id)));
        commands.push(Command::Jump(SessionId(id)));
    }
    let mut commands = commands.into_iter();
    let mut reassembler = Reassembler::default();
    let mut big = Reassembler::default();
    let mut checked = 0;

    for line in text.lines() {
        let fields: Vec<&str> = line.split(' ').collect();
        match fields[0] {
            "encode" => {
                let (_, frame, capacity) =
                    frames.iter().find(|(n, ..)| *n == fields[1]).expect(fields[1]);
                assert_eq!(capacity.to_string(), fields[2], "{line}");
                assert_eq!(hex(&encode(frame, *capacity)), fields[3], "encode {}", fields[1]);
            }
            "decode" => {
                let result = decode_frame(&unhex(fields[2]))
                    .map_or_else(|| "reject".into(), |frame| hex(&encode(&frame, MAX_FRAME_BYTES)));
                assert_eq!(result, fields[3], "decode {}", fields[1]);
            }
            "command" => {
                let command = commands.next().expect("more commands than expected");
                assert_eq!(hex(&encode_command(&command)), fields[1]);
            }
            "command-decode" => {
                let result = match decode_command(&unhex(fields[1])) {
                    Some(Command::Dismiss(id)) => format!("0 {}", id.0),
                    Some(Command::Jump(id)) => format!("1 {}", id.0),
                    None => "reject".into(),
                };
                assert_eq!(result, fields[2..].join(" "), "{line}");
            }
            "chunk" => {
                let result = reassembler
                    .push(&unhex(fields[1]))
                    .map_or_else(|| "none".into(), |f| format!("frame {}", hex(f)));
                assert_eq!(result, fields[2..].join(" "), "{line}");
            }
            "big" => {
                let i: u8 = fields[1].parse().unwrap();
                let mut chunk = vec![9, i, 8];
                chunk.extend(std::iter::repeat_n(0xAB, 250));
                let result =
                    big.push(&chunk).map_or_else(|| "none".into(), |f| f.len().to_string());
                assert_eq!(result, fields[2], "{line}");
            }
            other => panic!("unknown vector {other}"),
        }
        checked += 1;
    }
    assert!(checked > 40, "only {checked} vectors");
}

#[test]
fn decodes_the_golden_frame_the_mac_encodes() {
    let path = workspace().join("../protocol/golden/frame-v1.hex");
    let text = std::fs::read_to_string(path).expect("frame-v1.hex");
    let frame = decode_frame(&unhex(text.trim())).expect("decodes");
    assert_eq!(frame.seconds_of_day, 12 * 3600 + 34 * 60 + 56);
    assert_eq!(frame.night_start, 23);
    assert_eq!(frame.night_end, 8);
    assert!(!frame.settings.flip_screen);
    assert!(frame.settings.swap_buttons);
    assert_eq!(frame.settings.day_brightness, 80);
    assert_eq!(frame.settings.night_brightness, 12);
    let [a, b] = &frame.snapshot.sessions[..] else { panic!("two sessions") };
    assert_eq!(a.id.0, 0x0102_0304);
    assert_eq!(a.state, SessionState::NeedsYou);
    assert_eq!(a.in_state, Millis(42_000));
    assert_eq!(a.name, "servarium-05");
    assert_eq!(a.detail, "perm");
    assert_eq!(b.agent, Agent::Codex);
    assert_eq!(b.subagents, 3);
    assert_eq!(b.name, "Миграция базы");
}
