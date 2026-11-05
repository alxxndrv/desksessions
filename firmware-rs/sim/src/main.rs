//! Simulator: the firmware's app and renderer in a window.
//!
//!   ↑ / ↓   BOOT / GPIO21 (hold for a long press, both for the re-pair chord)
//!   A       add a session        X   remove the newest session
//!   N       a working session needs you
//!   P       pause / resume the scripted demo
//!   S       slow motion (¼ speed)
//!   L       drop / restore the link to the Mac (offline after 10 s)
//!   Z       drop the link as if 5 minutes ago (straight to asleep)
//!   U       forget pairing and walk through pairing again
//!   E       empty the list / bring it back
//!   R       start / stop recording a GIF into recordings/
//!
//! Run with --help for scripted recordings and stills.

use std::path::Path;
use std::process::ExitCode;
use std::time::Duration;

use desk_core::domain::Command;
use desk_sim::recorder::{Recorder, SCALE};
use desk_sim::{FPS, HEIGHT, Keys, Options, Simulation, WIDTH};
use minifb::{Key, KeyRepeat, Window, WindowOptions};

/// Reads this frame's keys from the window.
fn keys(window: &Window) -> (Keys, bool) {
    let pressed = |key| window.is_key_pressed(key, KeyRepeat::No);
    let keys = Keys {
        boot: window.is_key_down(Key::Up),
        aux: window.is_key_down(Key::Down),
        add: pressed(Key::A),
        remove: pressed(Key::X),
        escalate: pressed(Key::N),
        pause: pressed(Key::P),
        slow_motion: pressed(Key::S),
        link: pressed(Key::L),
        sleep: pressed(Key::Z),
        pair: pressed(Key::U),
        empty: pressed(Key::E),
    };
    (keys, pressed(Key::R))
}

/// The frame scaled up for the window, as 0RGB words.
fn window_buffer(rgb: &[u8], out: &mut Vec<u32>) {
    out.clear();
    for y in 0..HEIGHT * SCALE {
        for x in 0..WIDTH * SCALE {
            let i = ((y / SCALE) * WIDTH + x / SCALE) * 3;
            out.push(u32::from(rgb[i]) << 16 | u32::from(rgb[i + 1]) << 8 | u32::from(rgb[i + 2]));
        }
    }
}

fn describe(command: Command) -> String {
    match command {
        Command::Dismiss(id) => format!("[mac] dismiss session {}", id.0),
        Command::Jump(id) => format!("[mac] jump to session {}", id.0),
    }
}

fn main() -> ExitCode {
    let options = match Options::parse(std::env::args_os().skip(1)) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::from(2);
        }
    };
    if !options.shots_dir.is_empty()
        && let Err(e) = std::fs::create_dir_all(&options.shots_dir)
    {
        eprintln!("can't create {}: {e}", options.shots_dir);
        return ExitCode::FAILURE;
    }

    let capturing = options.capturing();
    let mut sim = Simulation::new(options.clone());
    let mut recorder = Recorder::default();
    if let Some(gif) = &options.gif
        && let Err(e) = recorder.start(gif, FPS)
    {
        eprintln!("can't start ffmpeg: {e}");
        return ExitCode::FAILURE;
    }

    let mut window = if options.headless {
        None
    } else {
        match Window::new("desksessions", WIDTH * SCALE, HEIGHT * SCALE, WindowOptions::default()) {
            Ok(window) => Some(window),
            Err(e) => {
                eprintln!("can't open a window: {e}");
                return ExitCode::FAILURE;
            }
        }
    };
    let mut pixels = Vec::new();
    let mut next_shot = 0;
    let mut clip = 0;
    let frame = Duration::from_micros(1_000_000 / u64::from(FPS));
    let mut next_frame = std::time::Instant::now();

    loop {
        let (keys, record) = window.as_ref().map_or((Keys::default(), false), keys);
        if record {
            if recorder.recording() {
                recorder.stop();
                println!("[sim] recording saved");
            } else {
                clip += 1;
                let path = format!("recordings/clip-{clip}.gif");
                let started =
                    std::fs::create_dir_all("recordings").and_then(|()| recorder.start(&path, FPS));
                match started {
                    Ok(()) => println!("[sim] recording {path}"),
                    Err(e) => eprintln!("can't record: {e}"),
                }
            }
        }

        if let Some(command) = sim.step(&keys) {
            println!("{}", describe(command));
        }
        let rgb = sim.rgb();
        if let Some(window) = window.as_mut() {
            window_buffer(&rgb, &mut pixels);
            if window.update_with_buffer(&pixels, WIDTH * SCALE, HEIGHT * SCALE).is_err()
                || !window.is_open()
            {
                return ExitCode::SUCCESS;
            }
        }
        recorder.frame(&rgb);

        let t = sim.since_launch();
        while let Some(&shot) = options.shots.get(next_shot).filter(|&&s| s <= t) {
            let path = Path::new(&options.shots_dir).join(format!("shot-{}.png", shot.count()));
            if let Err(e) = Recorder::still(&rgb, &path) {
                eprintln!("can't write {}: {e}", path.display());
            }
            next_shot += 1;
        }
        if options.stop_after.is_some_and(|stop| t >= stop) {
            recorder.stop();
            return ExitCode::SUCCESS;
        }

        if !capturing {
            next_frame += frame;
            if let Some(wait) = next_frame.checked_duration_since(std::time::Instant::now()) {
                std::thread::sleep(wait);
            }
        }
    }
}
