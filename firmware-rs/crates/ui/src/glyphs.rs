//! The little drawings: agent marks, the DONE check and the character.

use core::f32::consts::PI;

use desk_core::math::{self, lround};

use crate::canvas::{Canvas, Rgb888};

/// Claude: a four-point sparkle.
pub fn sparkle<B>(g: &mut Canvas<B>, cx: f32, cy: f32, radius: f32, angle: f32, color: Rgb888)
where
    B: AsRef<[u16]> + AsMut<[u16]>,
{
    for spoke in 0..4 {
        let a = angle + spoke as f32 * PI / 2.0;
        g.wedge(
            cx,
            cy,
            cx + math::cos(a) * radius,
            cy + math::sin(a) * radius,
            radius * 0.3,
            0.35,
            color,
        );
    }
}

/// Codex: a hexagon outline.
pub fn hexagon<B>(g: &mut Canvas<B>, cx: f32, cy: f32, radius: f32, angle: f32, color: Rgb888)
where
    B: AsRef<[u16]> + AsMut<[u16]>,
{
    for side in 0..6 {
        let a0 = angle + side as f32 * PI / 3.0;
        let a1 = a0 + PI / 3.0;
        g.line(
            cx + math::cos(a0) * radius,
            cy + math::sin(a0) * radius,
            cx + math::cos(a1) * radius,
            cy + math::sin(a1) * radius,
            radius * 0.15,
            color,
        );
    }
}

/// DONE: a check mark.
pub fn check<B>(g: &mut Canvas<B>, cx: f32, cy: f32, radius: f32, color: Rgb888)
where
    B: AsRef<[u16]> + AsMut<[u16]>,
{
    let w = radius * 0.18;
    let knee_x = cx - radius * 0.18;
    let knee_y = cy + radius * 0.48;
    g.line(cx - radius * 0.62, cy + radius * 0.02, knee_x, knee_y, w, color);
    g.line(knee_x, knee_y, cx + radius * 0.68, cy - radius * 0.52, w, color);
}

/// The little character on the quiet, offline and pairing screens.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Face {
    /// 0 closed (a line) … 1 open.
    pub eyes_open: f32,
    /// 0 flat … 1 smiling.
    pub smile: f32,
    /// Vertical scale for bounces.
    pub squash: f32,
}

impl Default for Face {
    fn default() -> Self {
        Self { eyes_open: 1.0, smile: 1.0, squash: 1.0 }
    }
}

pub fn face<B>(
    g: &mut Canvas<B>,
    cx: f32,
    cy: f32,
    radius: f32,
    face: &Face,
    body: Rgb888,
    features: Rgb888,
) where
    B: AsRef<[u16]> + AsMut<[u16]>,
{
    let ry = radius * face.squash;
    let bottom = cy + radius; // squash from the ground up
    let center_y = bottom - ry;
    // Body: a soft rounded blob, slightly wider than tall.
    g.fill_smooth_round_rect(
        lround(cx - radius * 1.15),
        lround(center_y - ry),
        lround(radius * 2.3),
        lround(ry * 2.0),
        lround(radius.min(ry) * 0.95),
        body,
    );

    // Eyes: dots that close into lines.
    let eye_y = center_y - ry * 0.12;
    let eye_dx = radius * 0.42;
    let eye_r = radius * 0.13;
    for side in [-1.0, 1.0] {
        let ex = cx + side * eye_dx;
        let open = face.eyes_open.clamp(0.0, 1.0);
        if open > 0.15 {
            let half = eye_r * open;
            g.line(ex, eye_y - half + eye_r * 0.6, ex, eye_y + half - eye_r * 0.6, eye_r, features);
        } else {
            g.line(
                ex - eye_r * 1.2,
                eye_y + eye_r * 0.4,
                ex + eye_r * 1.2,
                eye_y + eye_r * 0.4,
                eye_r * 0.45,
                features,
            );
        }
    }

    // Mouth: a small curve made of two short strokes.
    let mouth_y = center_y + ry * 0.32;
    let mw = radius * 0.28;
    let dip = radius * 0.16 * face.smile;
    g.line(cx - mw, mouth_y, cx, mouth_y + dip, radius * 0.06, features);
    g.line(cx, mouth_y + dip, cx + mw, mouth_y, radius * 0.06, features);
}
