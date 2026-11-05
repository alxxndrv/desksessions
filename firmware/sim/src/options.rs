//! Command line, flag for flag the same as the C++ simulator's.

use clap::Parser;
use desk_core::time::Millis;

/// What a scripted cue does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Up,
    Down,
    Add,
    Remove,
    Escalate,
    Pause,
    Link,
    Sleep,
    Pair,
    Empty,
}

/// A scripted input: press or hold a button, or poke the pretend Mac, at a time since launch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cue {
    pub at: Millis,
    pub action: Action,
    pub hold_for: Millis,
}

#[derive(Parser, Debug)]
#[command(name = "desk-sim", about = "The desk clock's firmware in a window.")]
struct Args {
    /// No scripted Mac activity.
    #[arg(long)]
    no_demo: bool,
    /// Show frames at full brightness (cleaner recordings).
    #[arg(long)]
    no_backlight: bool,
    /// Simulated speed, e.g. 0.25 for slow motion.
    #[arg(long, value_name = "F")]
    slowmo: Option<f64>,
    /// Record to this file (.gif or .mp4), then quit after --seconds.
    #[arg(long, value_name = "PATH")]
    gif: Option<String>,
    /// Quit after this much simulated time.
    #[arg(long, value_name = "N")]
    seconds: Option<f64>,
    /// Directory for --at stills.
    #[arg(long, value_name = "DIR")]
    shots: Option<String>,
    /// PNG stills at these simulated milliseconds, then quit.
    #[arg(long, value_name = "T1,T2,…")]
    at: Option<String>,
    /// Pretend it's this time of day (night mode is 23:00–08:00).
    #[arg(long, value_name = "HH:MM")]
    time: Option<String>,
    /// "T:act,…" with act ∈ up, down, longup, longdown, add, remove, needs, pause, link, sleep,
    /// pair, empty.
    #[arg(long, value_name = "CUES")]
    script: Option<String>,
    /// No window: run as fast as possible (recordings and stills only).
    #[arg(long)]
    headless: bool,
    /// Frame rate and drawing time over the picture, as the device's `overlay` build shows.
    #[arg(long)]
    overlay: bool,
}

/// Everything the simulator was asked to do.
#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    /// Scripted Mac activity.
    pub demo: bool,
    /// Below 1 is slow motion.
    pub speed: f64,
    pub gif: Option<String>,
    pub stop_after: Option<Millis>,
    pub shots_dir: String,
    pub shots: Vec<Millis>,
    pub cues: Vec<Cue>,
    pub seconds_of_day: Option<u32>,
    pub backlight: bool,
    pub headless: bool,
    pub overlay: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            demo: true,
            speed: 1.0,
            gif: None,
            stop_after: None,
            shots_dir: String::new(),
            shots: Vec::new(),
            cues: Vec::new(),
            seconds_of_day: None,
            backlight: true,
            headless: false,
            overlay: false,
        }
    }
}

/// `atoll`: leading digits, 0 if none.
fn leading_int(text: &str) -> i64 {
    let text = text.trim_start();
    let (sign, digits) = match text.strip_prefix('-') {
        Some(rest) => (-1, rest),
        None => (1, text.strip_prefix('+').unwrap_or(text)),
    };
    let end = digits.find(|c: char| !c.is_ascii_digit()).unwrap_or(digits.len());
    sign * digits[..end].parse::<i64>().unwrap_or(0)
}

fn parse_cue(text: &str) -> Option<Cue> {
    let (at, act) = text.split_once(':')?;
    const LONG: Millis = Millis(800);
    let (action, hold_for) = match act {
        "up" => (Action::Up, Millis(90)),
        "down" => (Action::Down, Millis(90)),
        "longup" => (Action::Up, LONG),
        "longdown" => (Action::Down, LONG),
        "add" => (Action::Add, Millis(90)),
        "remove" => (Action::Remove, Millis(90)),
        "needs" => (Action::Escalate, Millis(90)),
        "pause" => (Action::Pause, Millis(90)),
        "link" => (Action::Link, Millis(90)),
        "sleep" => (Action::Sleep, Millis(90)),
        "pair" => (Action::Pair, Millis(90)),
        "empty" => (Action::Empty, Millis(90)),
        _ => return None,
    };
    Some(Cue { at: Millis(leading_int(at)), action, hold_for })
}

fn parse_time(text: &str) -> Option<u32> {
    let (hour, minute) = text.split_once(':')?;
    let hour: i64 = hour.trim().parse().ok()?;
    let minute = leading_int(minute);
    Some((hour * 3600 + minute * 60) as u32)
}

impl Options {
    /// Parses a command line (without the program name).
    ///
    /// # Errors
    ///
    /// A message for the user when a flag or value is malformed.
    pub fn parse<I, S>(args: I) -> Result<Self, String>
    where
        I: IntoIterator<Item = S>,
        S: Into<std::ffi::OsString> + Clone,
    {
        let args = Args::try_parse_from(
            std::iter::once(std::ffi::OsString::from("desk-sim"))
                .chain(args.into_iter().map(Into::into)),
        )
        .map_err(|e| e.to_string())?;
        let mut options = Self {
            demo: !args.no_demo,
            speed: args.slowmo.unwrap_or(1.0),
            gif: args.gif,
            stop_after: args.seconds.map(|s| Millis((s * 1000.0) as i64)),
            shots_dir: args.shots.unwrap_or_default(),
            backlight: !args.no_backlight,
            headless: args.headless,
            overlay: args.overlay,
            ..Self::default()
        };
        if let Some(at) = args.at {
            options.shots =
                at.split(',').filter(|t| !t.is_empty()).map(|t| Millis(leading_int(t))).collect();
        }
        if let Some(time) = args.time {
            options.seconds_of_day = Some(parse_time(&time).ok_or("bad --time, want HH:MM")?);
        }
        if let Some(script) = args.script {
            for item in script.split(',').filter(|i| !i.is_empty()) {
                options.cues.push(parse_cue(item).ok_or_else(|| format!("bad cue: {item}"))?);
            }
        }
        if !options.shots.is_empty() && options.stop_after.is_none() {
            let last = options.shots.iter().copied().max().unwrap_or_default();
            options.stop_after = Some(last + Millis(1));
        }
        Ok(options)
    }

    /// Frames are produced for files, so time advances exactly one frame per frame.
    #[must_use]
    pub fn capturing(&self) -> bool {
        self.gif.is_some() || !self.shots.is_empty()
    }

    /// Whether a scripted cue is holding `action`'s button at `t`.
    #[must_use]
    pub fn cue_holds(&self, action: Action, t: Millis) -> bool {
        self.cues.iter().any(|c| c.action == action && t >= c.at && t < c.at + c.hold_for)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_c_simulators_flags() {
        let options = Options::parse([
            "--no-backlight",
            "--time",
            "23:30",
            "--script",
            "500:down,1200:longdown,3000:sleep",
            "--shots",
            "out",
            "--at",
            "100,2500",
        ])
        .unwrap();
        assert!(!options.backlight);
        assert_eq!(options.seconds_of_day, Some(23 * 3600 + 30 * 60));
        assert_eq!(options.cues.len(), 3);
        assert_eq!(
            options.cues[1],
            Cue { at: Millis(1200), action: Action::Down, hold_for: Millis(800) }
        );
        assert_eq!(options.shots, [Millis(100), Millis(2500)]);
        assert_eq!(options.stop_after, Some(Millis(2501)));
        assert!(options.capturing());
    }

    #[test]
    fn rejects_unknown_cues() {
        assert!(Options::parse(["--script", "10:jump"]).is_err());
        assert!(Options::parse(["--time", "noon"]).is_err());
    }
}
