//! Every golden scenario (`golden/scenarios.txt`), run in the Rust simulator, against what the
//! C++ firmware did:
//!  - the app's animated state on every frame (`golden/fixtures/trace-*.txt`), and
//!  - the stills (`golden/frames/*/shot-*.png`): a frame passes when under 0.2 % of its pixels
//!    differ by more than 2 levels in any channel.
//!
//! Failing stills are written with a difference image to `target/golden-diff/`.

use std::path::{Path, PathBuf};

use desk_sim::recorder::{read_png, write_png};
use desk_sim::{HEIGHT, Keys, Options, Simulation, WIDTH};

const LEVELS: u8 = 2;
const MAX_DIFFERING: usize = WIDTH * HEIGHT / 500; // 0.2 %

fn workspace() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn scenarios() -> Vec<(String, Vec<String>)> {
    let text =
        std::fs::read_to_string(workspace().join("golden/scenarios.txt")).expect("scenarios");
    text.lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .map(|l| {
            let (name, args) = l.split_once(':').unwrap();
            (name.trim().to_string(), args.split_whitespace().map(String::from).collect())
        })
        .collect()
}

/// Whether two trace lines agree: same words, numbers within 1e-3 (or 1e-4 relative).
fn same_state(ours: &str, theirs: &str) -> Result<(), String> {
    let a: Vec<&str> = ours.split(' ').collect();
    let b: Vec<&str> = theirs.split(' ').collect();
    if a.len() != b.len() {
        return Err(format!("{} words vs {}", a.len(), b.len()));
    }
    for (i, (x, y)) in a.iter().zip(&b).enumerate() {
        match (x.parse::<f64>(), y.parse::<f64>()) {
            (Ok(x), Ok(y)) => {
                let d = (x - y).abs();
                if d > 1e-3 && d > 1e-4 * x.abs().max(y.abs()) {
                    return Err(format!(
                        "word {i} ({}): {x} vs {y}",
                        a[..i].iter().rev().find(|w| w.parse::<f64>().is_err()).unwrap_or(&"?")
                    ));
                }
            }
            _ if x == y => {}
            _ => return Err(format!("word {i}: {x} vs {y}")),
        }
    }
    Ok(())
}

/// Pixels differing by more than [`LEVELS`] in any channel, and a picture of where.
fn compare(ours: &[u8], theirs: &[u8]) -> (usize, Vec<u8>) {
    let mut differing = 0;
    let mut diff = vec![0; ours.len()];
    for (i, (a, b)) in ours.chunks(3).zip(theirs.chunks(3)).enumerate() {
        let worst = a.iter().zip(b).map(|(x, y)| x.abs_diff(*y)).max().unwrap_or(0);
        if worst > LEVELS {
            differing += 1;
            diff[i * 3] = 255;
        } else {
            diff[i * 3 + 1] = a[1] / 3;
        }
    }
    (differing, diff)
}

/// How close the stills came, for the summary.
#[derive(Default)]
struct Tally {
    stills: usize,
    identical: usize,
    worst: usize,
    any_level: usize,
}

fn run_scenario(name: &str, args: &[String], failures: &mut Vec<String>, tally: &mut Tally) {
    let options = Options::parse(args).unwrap();
    let shots = options.shots.clone();
    let stop = options.stop_after.unwrap();
    let trace_path = workspace().join(format!("golden/fixtures/trace-{name}.txt"));
    let trace = std::fs::read_to_string(&trace_path).unwrap_or_default();
    // The C++ pretend Mac prints what it hears into the same stream: not state.
    let mut trace = trace.lines().filter(|l| l.starts_with("t "));
    let frames = workspace().join("golden/frames").join(name);
    let diffs = workspace().join("target/golden-diff").join(name);

    let mut sim = Simulation::new(options);
    let mut next = 0;
    let mut trace_ok = true;
    loop {
        sim.step(&Keys::default());
        let t = sim.since_launch();
        if trace_ok
            && let Some(theirs) = trace.next()
            && let Err(e) = same_state(&sim.trace_line(), theirs)
        {
            failures.push(format!("{name} t={}: state differs, {e}", t.count()));
            trace_ok = false; // the first divergence is the one worth reading
        }
        while let Some(&shot) = shots.get(next).filter(|&&s| s <= t) {
            next += 1;
            let file = format!("shot-{}.png", shot.count());
            let Ok((w, h, theirs)) = read_png(&frames.join(&file)) else {
                failures.push(format!("{name}/{file}: no reference (run `cargo xtask reference`)"));
                continue;
            };
            assert_eq!((w, h), (WIDTH, HEIGHT));
            let ours = sim.rgb();
            let (differing, diff) = compare(&ours, &theirs);
            tally.stills += 1;
            tally.identical += usize::from(ours == theirs);
            tally.worst = tally.worst.max(differing);
            tally.any_level = tally
                .any_level
                .max(ours.chunks(3).zip(theirs.chunks(3)).filter(|(a, b)| a != b).count());
            if differing > MAX_DIFFERING {
                failures.push(format!("{name}/{file}: {differing} pixels differ"));
                let _ = std::fs::create_dir_all(&diffs);
                let _ = write_png(&diffs.join(format!("ours-{file}")), WIDTH, HEIGHT, &ours);
                let _ = write_png(&diffs.join(format!("theirs-{file}")), WIDTH, HEIGHT, &theirs);
                let _ = write_png(&diffs.join(format!("diff-{file}")), WIDTH, HEIGHT, &diff);
            }
        }
        if t >= stop {
            break;
        }
    }
}

#[test]
fn scenarios_match_the_c_firmware() {
    let _ = std::fs::remove_dir_all(workspace().join("target/golden-diff"));
    let mut failures = Vec::new();
    let scenarios = scenarios();
    assert!(!scenarios.is_empty());
    let mut tally = Tally::default();
    for (name, args) in &scenarios {
        run_scenario(name, args, &mut failures, &mut tally);
    }
    eprintln!(
        "{} stills: {} identical; most pixels beyond ±{LEVELS} in one still: {}; most pixels \
         differing at all: {}",
        tally.stills, tally.identical, tally.worst, tally.any_level
    );
    assert!(failures.is_empty(), "{} failures:\n{}", failures.len(), failures.join("\n"));
}

#[test]
fn every_scenario_has_its_references() {
    for (name, args) in scenarios() {
        let options = Options::parse(&args).unwrap();
        let dir = workspace().join("golden/frames").join(&name);
        for shot in options.shots {
            let path = dir.join(format!("shot-{}.png", shot.count()));
            assert!(Path::new(&path).exists(), "{}", path.display());
        }
    }
}
