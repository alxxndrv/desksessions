//! Project chores.
//!
//!   cargo xtask check       every gate: format, lints, tests, and the device build when the
//!                           `esp` toolchain is installed
//!   cargo xtask bless       re-record the golden stills from the simulator, after an
//!                           intentional change to how things look

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use desk_sim::recorder::write_png;
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

fn main() -> ExitCode {
    let task = std::env::args().nth(1).unwrap_or_default();
    let result = match task.as_str() {
        "check" => check(),
        "bless" => bless(),
        _ => Err("usage: cargo xtask check | bless".into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("xtask {task}: {message}");
            ExitCode::FAILURE
        }
    }
}
