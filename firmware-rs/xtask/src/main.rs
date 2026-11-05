//! Project chores.
//!
//!   cargo xtask check       every gate: format, lints, tests, and the device build when the
//!                           `esp` toolchain is installed
//!   cargo xtask bless       re-record the golden stills from the Rust simulator, after an
//!                           intentional change to how things look
//!   cargo xtask reference   regenerate wire vectors, curves, traces and stills from the C++
//!                           firmware (needs ../firmware, PlatformIO and clang)

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use desk_sim::recorder::{SCALE, read_png, write_png};
use desk_sim::{HEIGHT, Options, Simulation, WIDTH};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("workspace root").to_path_buf()
}

fn golden() -> PathBuf {
    root().join("golden")
}

/// `(name, arguments)` for every line of `golden/scenarios.txt`.
fn scenarios() -> Vec<(String, Vec<String>)> {
    let text = std::fs::read_to_string(golden().join("scenarios.txt")).expect("scenarios.txt");
    text.lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .map(|l| {
            let (name, args) = l.split_once(':').expect("name: args");
            (name.trim().to_string(), args.split_whitespace().map(String::from).collect())
        })
        .collect()
}

fn run(command: &mut Command) -> Result<(), String> {
    let status = command.status().map_err(|e| format!("{command:?}: {e}"))?;
    if status.success() { Ok(()) } else { Err(format!("{command:?} failed: {status}")) }
}

fn cargo() -> Command {
    let mut c = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()));
    c.current_dir(root());
    c
}

fn check() -> Result<(), String> {
    run(cargo().args(["fmt", "--all", "--check"]))?;
    run(cargo().args(["clippy", "--workspace", "--all-targets", "--", "-D", "warnings"]))?;
    run(cargo().args(["test", "--workspace", "--quiet"]))?;
    let device = root().join("device");
    let esp = Command::new("rustup")
        .args(["run", "esp", "rustc", "--version"])
        .output()
        .is_ok_and(|o| o.status.success());
    if esp {
        // The device crate picks the `esp` toolchain from its own rust-toolchain.toml, so the
        // toolchain this xtask runs under must not leak in; espup's script puts the Xtensa linker
        // on the PATH.
        let device_cargo = |args: &str| {
            let mut c = Command::new("sh");
            c.current_dir(&device)
                .env_remove("RUSTUP_TOOLCHAIN")
                .env_remove("CARGO")
                .arg("-c")
                .arg(format!(
                    "[ -f \"$HOME/export-esp.sh\" ] && . \"$HOME/export-esp.sh\"; cargo {args}"
                ));
            c
        };
        run(&mut device_cargo("fmt --check"))?;
        run(&mut device_cargo("clippy --release -- -D warnings"))?;
        run(&mut device_cargo("clippy --release --features demo,stats -- -D warnings"))?;
        run(&mut device_cargo("build --release"))?;
        run(&mut device_cargo("build --release --features demo"))?;
    } else {
        eprintln!("device build skipped: no `esp` toolchain (install with `espup install`)");
    }
    println!("all gates passed");
    Ok(())
}

/// Runs a scenario in the Rust simulator and saves its stills.
fn bless() -> Result<(), String> {
    for (name, args) in scenarios() {
        let options = Options::parse(&args)?;
        let shots = options.shots.clone();
        let stop = options.stop_after.ok_or("scenario without --at")?;
        let dir = golden().join("frames").join(&name);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let mut sim = Simulation::new(options);
        let mut next = 0;
        loop {
            sim.step(&desk_sim::Keys::default());
            let t = sim.since_launch();
            while let Some(&shot) = shots.get(next).filter(|&&s| s <= t) {
                let path = dir.join(format!("shot-{}.png", shot.count()));
                write_png(&path, WIDTH, HEIGHT, &sim.rgb()).map_err(|e| e.to_string())?;
                next += 1;
            }
            if t >= stop {
                break;
            }
        }
        println!("blessed {name}: {} stills", shots.len());
    }
    Ok(())
}

fn reference() -> Result<(), String> {
    let firmware = root().parent().ok_or("no parent")?.join("firmware");
    if !firmware.is_dir() {
        return Err(format!("{} not found: the C++ firmware is the reference", firmware.display()));
    }
    let out = root().join("target").join("reference");
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;

    // The C++ core, demo and option parser, with the dumper on top.
    let etl = firmware.join(".pio/libdeps/native_test/Embedded Template Library/include");
    if !etl.is_dir() {
        run(Command::new("pio").current_dir(&firmware).args([
            "pkg",
            "install",
            "-e",
            "native_test",
        ]))?;
    }
    let mut sources: Vec<PathBuf> = Vec::new();
    for dir in ["lib/core/src", "lib/demo/src"] {
        collect_cpp(&firmware.join(dir), &mut sources);
    }
    sources.push(firmware.join("src/sim/options.cpp"));
    sources.push(golden().join("reference/dump.cpp"));
    let dumper = out.join("desk-dump");
    run(Command::new("clang++")
        .args(["-std=c++20", "-O2", "-w"])
        .arg("-I")
        .arg(firmware.join("lib/core/include"))
        .arg("-I")
        .arg(firmware.join("lib/demo/include"))
        .arg("-I")
        .arg(firmware.join("src/sim"))
        .arg("-I")
        .arg(&etl)
        .args(&sources)
        .arg("-o")
        .arg(&dumper))?;

    let fixtures = golden().join("fixtures");
    std::fs::create_dir_all(&fixtures).map_err(|e| e.to_string())?;
    for what in ["wire", "curves"] {
        let output = Command::new(&dumper).arg(what).output().map_err(|e| e.to_string())?;
        std::fs::write(fixtures.join(format!("{what}.txt")), output.stdout)
            .map_err(|e| e.to_string())?;
    }
    for (name, args) in scenarios() {
        let output =
            Command::new(&dumper).arg("trace").args(&args).output().map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err(format!("trace {name}: {}", String::from_utf8_lossy(&output.stderr)));
        }
        std::fs::write(fixtures.join(format!("trace-{name}.txt")), output.stdout)
            .map_err(|e| e.to_string())?;
    }
    println!("wire, curves and traces written to {}", fixtures.display());

    // The C++ simulator's stills, scaled back down to one pixel per pixel.
    run(Command::new("pio").current_dir(&firmware).args(["run", "-e", "sim"]))?;
    let program = firmware.join(".pio/build/sim/program");
    for (name, args) in scenarios() {
        let shots = out.join("shots").join(&name);
        let _ = std::fs::remove_dir_all(&shots);
        let args: Vec<String> = args
            .iter()
            .map(|a| if a == "." { shots.display().to_string() } else { a.clone() })
            .collect();
        let expected = Options::parse(&args)?.shots.len();
        let status = Command::new(&program).args(&args).status().map_err(|e| e.to_string())?;
        let written = std::fs::read_dir(&shots).map(Iterator::count).unwrap_or(0);
        // The C++ simulator calls exit() from inside SDL's loop and sometimes crashes on the way
        // out, after its stills are written. Only missing stills count as failure.
        if written != expected {
            return Err(format!("{name}: {written} of {expected} stills ({status})"));
        }
        let dir = golden().join("frames").join(&name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        for entry in std::fs::read_dir(&shots).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            let (w, h, rgb) = read_png(&path).map_err(|e| e.to_string())?;
            if (w, h) != (WIDTH * SCALE, HEIGHT * SCALE) {
                return Err(format!("{}: {w}×{h}", path.display()));
            }
            let mut small = Vec::with_capacity(WIDTH * HEIGHT * 3);
            for y in 0..HEIGHT {
                for x in 0..WIDTH {
                    let i = (y * SCALE * w + x * SCALE) * 3;
                    small.extend_from_slice(&rgb[i..i + 3]);
                }
            }
            let file = path.file_name().ok_or("no file name")?;
            write_png(&dir.join(file), WIDTH, HEIGHT, &small).map_err(|e| e.to_string())?;
        }
        println!("reference stills for {name}");
    }
    Ok(())
}

fn collect_cpp(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_cpp(&path, out);
        } else if path.extension().is_some_and(|e| e == "cpp") {
            out.push(path);
        }
    }
}

fn main() -> ExitCode {
    let task = std::env::args().nth(1).unwrap_or_default();
    let result = match task.as_str() {
        "check" => check(),
        "bless" => bless(),
        "reference" => reference(),
        _ => Err("usage: cargo xtask check | bless | reference".into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("xtask {task}: {message}");
            ExitCode::FAILURE
        }
    }
}
