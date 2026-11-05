use super::easing::lerp;
use crate::time::Millis;

/// Pebble's design frame: its hand-tuned curves are tables of one value per 33 ms frame.
pub const PEBBLE_FRAME: Millis = Millis(33);

/// A Pebble "moook" motion: a few frames of anticipation near the start, optional linear middle
/// frames, then frames of overshoot past the target that settle back onto it.
///
/// Offsets are in pixels, as in `PebbleOS` (`fw/applib/ui/animation_interpolate.c`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MoookShape {
    /// Added to `from`, along the direction of travel.
    pub r#in: &'static [i16],
    /// Added to `to`, past the target.
    pub out: &'static [i16],
    pub mid_frames: u8,
}

pub const MOOOK_IN: [i16; 3] = [0, 1, 20];
pub const MOOOK_OUT: [i16; 4] = [4, 2, 1, 0];
pub const PEEK_OUT: [i16; 5] = [8, 4, 2, 1, 0];

/// 7 frames ≈ 231 ms. Cursor moves, window slides, row exits.
pub const MOOOK: MoookShape = MoookShape { r#in: &MOOOK_IN, out: &MOOOK_OUT, mid_frames: 0 };
/// 10 frames ≈ 330 ms. Pebble's timeline scroll; our reconnect "wake".
pub const MOOOK_SOFT: MoookShape = MoookShape { r#in: &MOOOK_IN, out: &MOOOK_OUT, mid_frames: 3 };
/// Doubled bounce from Pebble's timeline peek; the NEEDS YOU glyph hop.
pub const PEEK: MoookShape = MoookShape { r#in: &MOOOK_IN, out: &PEEK_OUT, mid_frames: 0 };

/// Distance at which the pixel tables apply unscaled. Shorter moves get proportionally smaller
/// anticipation and overshoot so a 5 px move cannot jump 20 px past its target.
pub const MOOOK_REFERENCE_DISTANCE: f32 = 30.0;

impl MoookShape {
    #[must_use]
    pub const fn frames(&self) -> usize {
        self.r#in.len() + self.mid_frames as usize + self.out.len()
    }

    #[must_use]
    pub const fn duration(&self) -> Millis {
        Millis(PEBBLE_FRAME.0 * self.frames() as i64)
    }
}

struct Keyframes<'a> {
    shape: &'a MoookShape,
    from: f32,
    to: f32,
    /// ±scale: sign of travel times the short-move scale factor.
    direction: f32,
}

impl Keyframes<'_> {
    /// Value of frame `index`; one past the last frame is the target itself.
    fn at(&self, index: usize) -> f32 {
        let shape = self.shape;
        let in_count = shape.r#in.len();
        let mid_end = in_count + shape.mid_frames as usize;
        if index >= shape.frames() {
            return self.to;
        }
        if index < in_count {
            return self.from + self.direction * f32::from(shape.r#in[index]);
        }
        if index < mid_end {
            let start = match shape.r#in.last() {
                Some(&last) => self.from + self.direction * f32::from(last),
                None => self.from,
            };
            let end = self.to + self.direction * f32::from(shape.out[0]);
            let step = (index - in_count + 1) as f32;
            return lerp(start, end, step / f32::from(shape.mid_frames + 1));
        }
        self.to + self.direction * f32::from(shape.out[index - mid_end])
    }
}

/// Position at `elapsed` of a moook move from `from` to `to`. Pebble steps from frame to frame;
/// this interpolates between neighbouring frames so the same motion renders smoothly at 60 fps.
#[must_use]
pub fn moook(shape: &MoookShape, from: f32, to: f32, elapsed: Millis) -> f32 {
    let distance = to - from;
    if distance == 0.0 || elapsed >= shape.duration() {
        return to;
    }
    if elapsed.0 <= 0 {
        return from;
    }

    let scale = (distance.abs() / MOOOK_REFERENCE_DISTANCE).min(1.0);
    let frames = Keyframes { shape, from, to, direction: scale.copysign(distance) };

    let position = elapsed.0 as f32 / PEBBLE_FRAME.0 as f32;
    let index = position as usize;
    let within = position - index as f32;
    lerp(frames.at(index), frames.at(index + 1), within)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assert_float_eq;

    #[test]
    fn duration_is_seven_pebble_frames() {
        assert_eq!(MOOOK.duration(), Millis(231));
        assert_eq!(MOOOK_SOFT.duration(), Millis(330));
        assert_eq!(PEEK.duration(), Millis(264));
    }

    #[test]
    fn hits_pebble_keyframes_on_frame_boundaries() {
        // A one-row move down: 0 → 30 px.
        let expected = [0.0, 1.0, 20.0, 34.0, 32.0, 31.0, 30.0];
        for (frame, want) in expected.iter().enumerate() {
            assert_float_eq!(
                moook(&MOOOK, 0.0, 30.0, PEBBLE_FRAME * frame as i64),
                *want,
                "frame {frame}"
            );
        }
        assert_float_eq!(moook(&MOOOK, 0.0, 30.0, MOOOK.duration()), 30.0);
    }

    #[test]
    fn interpolates_between_frames() {
        assert_float_eq!(
            moook(&MOOOK, 0.0, 30.0, PEBBLE_FRAME + Millis(16)),
            1.0 + 19.0 * 16.0 / 33.0
        );
    }

    #[test]
    fn moves_upward_mirror_image() {
        assert_float_eq!(moook(&MOOOK, 30.0, 0.0, PEBBLE_FRAME * 2), 10.0);
        assert_float_eq!(moook(&MOOOK, 30.0, 0.0, PEBBLE_FRAME * 3), -4.0);
    }

    #[test]
    fn short_moves_scale_their_overshoot() {
        // 15 px is half the reference distance, so the 4 px overshoot becomes 2 px.
        assert_float_eq!(moook(&MOOOK, 0.0, 15.0, PEBBLE_FRAME * 3), 17.0);
        assert_float_eq!(moook(&MOOOK, 5.0, 5.0, Millis(50)), 5.0);
    }

    #[test]
    fn soft_variant_runs_linear_middle_frames() {
        // in {0,1,20}, then 3 frames from 20 to 34, then out {4,2,1,0}.
        assert_float_eq!(moook(&MOOOK_SOFT, 0.0, 30.0, PEBBLE_FRAME * 3), 23.5);
        assert_float_eq!(moook(&MOOOK_SOFT, 0.0, 30.0, PEBBLE_FRAME * 5), 30.5);
        assert_float_eq!(moook(&MOOOK_SOFT, 0.0, 30.0, PEBBLE_FRAME * 6), 34.0);
    }
}
