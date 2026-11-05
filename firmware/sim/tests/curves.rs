//! Animation curves against the C++ firmware's, sampled every millisecond
//! (`golden/fixtures/curves.txt`). Values must agree within 1e-4 (or 1e-5 relative).
//!
//! Channel velocities are a 1 ms difference of two positions times 1000, so they get 1000× the
//! tolerance: one unit in the last place of a 60 px position is already 0.008 px/s.

use std::collections::HashMap;
use std::path::PathBuf;

use desk_core::anim::{
    BOUNCY, CA_DEFAULT, Channel, CubicBezier, Curve, MOOOK, MOOOK_SETTLE, MOOOK_SOFT, MoookShape,
    Motion, PEEK, RUBBER_BAND_C, SMOOTH, SNAPPY, Spring, SpringParams, breathe, ease,
    marquee_offset, moook, rubber_band, spring_from,
};
use desk_core::time::{Instant, Millis, progress};

#[derive(Default)]
struct Rows(Vec<(String, i64, f32)>);

impl Rows {
    fn row(&mut self, name: &str, i: i64, value: f32) {
        self.0.push((name.to_string(), i, value));
    }
}

#[allow(clippy::too_many_lines)]
fn curves() -> Rows {
    let mut out = Rows::default();
    let named = [
        ("linear", Curve::Linear),
        ("ease-in", Curve::EaseIn),
        ("ease-out", Curve::EaseOut),
        ("ease-in-out", Curve::EaseInOut),
        ("ca-default", Curve::CaDefault),
        ("ca-ease-in", Curve::CaEaseIn),
        ("ca-ease-out", Curve::CaEaseOut),
        ("ca-ease-in-out", Curve::CaEaseInOut),
    ];
    for (name, curve) in named {
        for i in -10..=1010 {
            out.row(name, i, ease(curve, i as f32 / 1000.0));
        }
    }
    let back_out = CubicBezier::new(0.34, 1.56, 0.64, 1.0);
    for i in 0..=1000 {
        out.row("back-out", i, back_out.at(i as f32 / 1000.0));
    }
    assert!((CA_DEFAULT.at(0.5) - 0.8024).abs() < 2e-3);

    let moves: [(&str, &MoookShape, f32, f32); 9] = [
        ("moook-30", &MOOOK, 0.0, 30.0),
        ("moook-up", &MOOOK, 30.0, 0.0),
        ("moook-15", &MOOOK, 0.0, 15.0),
        ("moook-exit", &MOOOK, 0.0, -320.0),
        ("moook-soft", &MOOOK_SOFT, -14.0, 0.0),
        ("peek", &PEEK, -5.0, 0.0),
        ("settle", &MOOOK_SETTLE, 12.5, 60.0),
        ("moook-digit", &MOOOK, -24.0, 0.0),
        ("moook-none", &MOOOK, 5.0, 5.0),
    ];
    for (name, shape, from, to) in moves {
        for ms in -5..=340 {
            out.row(name, ms, moook(shape, from, to, Millis(ms)));
        }
    }

    let springs: [(&str, SpringParams, f32, f32, f32); 5] = [
        ("spring-smooth", SMOOTH, 0.0, 0.0, 100.0),
        ("spring-snappy", SNAPPY, 0.0, 0.0, 100.0),
        ("spring-bouncy", BOUNCY, 0.0, 0.0, 100.0),
        ("spring-wiggle", spring_from(0.18, 0.55), 0.0, 260.0, 0.0),
        ("spring-kick", BOUNCY, 1.0, 1.2, 1.0),
    ];
    for (name, params, value, velocity, target) in springs {
        let mut spring = Spring { value, velocity, target };
        for i in 0..400 {
            spring.step(&params);
            out.row(name, i, spring.value);
        }
    }
    out.row("params-smooth-k", 0, SMOOTH.stiffness);
    out.row("params-smooth-d", 0, SMOOTH.damping);
    out.row("params-snappy-d", 0, SNAPPY.damping);
    out.row("params-bouncy-d", 0, BOUNCY.damping);

    for ms in -2000..=4000 {
        out.row("breathe", ms, breathe(Millis(ms)));
    }
    for overflow in [50.0_f32, 13.5, 120.0] {
        let name = format!("marquee-{}", (overflow * 10.0) as i32);
        for ms in (0..=9000).step_by(3) {
            out.row(&name, ms, marquee_offset(overflow, Millis(ms)));
        }
    }
    for i in -200..=200 {
        out.row("rubber-band", i, rubber_band(i as f32 * 1.5, 100.0, RUBBER_BAND_C));
    }
    for i in -10..=1200 {
        out.row("progress", i, progress(Millis(i), Millis(1000)));
    }

    type Script = fn(&mut Channel, Instant);
    let scripts: [(&str, Script); 5] = [
        ("channel-tween-reverse", |c, t| {
            c.animate_to(100.0, Motion::tween(Curve::EaseInOut, Millis(250)), t)
        }),
        ("channel-spring", |c, t| c.animate_to(100.0, Motion::springy(SNAPPY), t)),
        ("channel-moook", |c, t| c.animate_to(30.0, Motion::moook(&MOOOK), t)),
        ("channel-kick", |c, t| c.kick(1.2, t)),
        ("channel-then", |c, t| {
            c.animate_to(-5.0, Motion::tween(Curve::EaseOut, Millis(48)), t);
            c.then(0.0, Motion::tween(Curve::EaseOut, Millis(144)));
        }),
    ];
    for (name, script) in scripts {
        let mut channel = Channel::new(0.0);
        let mut now = Instant::from_millis(1_000);
        script(&mut channel, now);
        for frame in 0..120_i64 {
            now += Millis(if frame % 3 == 2 { 17 } else { 16 });
            match frame {
                7 => channel.animate_to(
                    if frame % 2 == 1 { 0.0 } else { 12.0 },
                    Motion::springy(SMOOTH),
                    now,
                ),
                30 => channel.animate_to(60.0, Motion::moook(&MOOOK), now),
                33 => channel.animate_to(90.0, Motion::moook(&MOOOK), now),
                60 => channel.animate_to(10.0, Motion::tween(Curve::CaEaseInOut, Millis(400)), now),
                66 => {
                    channel.animate_to(-40.0, Motion::tween(Curve::CaEaseInOut, Millis(400)), now)
                }
                _ => {}
            }
            channel.update(now);
            out.row(name, frame, channel.value());
            out.row(&format!("{name}-velocity"), frame, channel.velocity());
        }
    }
    out
}

fn close(a: f32, b: f32, scale: f64) -> bool {
    let (a, b) = (f64::from(a), f64::from(b));
    (a - b).abs() <= 1e-4 * scale || (a - b).abs() <= 1e-5 * scale * a.abs().max(b.abs())
}

#[test]
fn curves_match_the_c_firmware() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../golden/fixtures/curves.txt");
    let text = std::fs::read_to_string(path).expect("curves.txt");
    let mut reference: HashMap<(String, i64), f32> = HashMap::new();
    for line in text.lines() {
        let mut fields = line.split(' ');
        let name = fields.next().unwrap().to_string();
        let i: i64 = fields.next().unwrap().parse().unwrap();
        let value: f32 = fields.next().unwrap().parse().unwrap();
        reference.insert((name, i), value);
    }

    let ours = curves();
    assert_eq!(ours.0.len(), reference.len(), "same samples");
    let mut failures = Vec::new();
    let mut worst = 0.0_f64;
    for (name, i, value) in &ours.0 {
        let want = reference[&(name.clone(), *i)];
        if !name.ends_with("-velocity") {
            worst = worst.max((f64::from(*value) - f64::from(want)).abs());
        }
        let scale = if name.ends_with("-velocity") { 1000.0 } else { 1.0 };
        if !close(*value, want, scale) {
            failures.push(format!("{name} {i}: {value} vs C++ {want}"));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} samples differ:\n{}",
        failures.len(),
        ours.0.len(),
        failures[..failures.len().min(20)].join("\n")
    );
    eprintln!("{} samples, largest difference {worst:e}", ours.0.len());
}
