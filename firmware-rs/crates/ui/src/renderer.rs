//! Draws one frame of the app.

use core::f32::consts::PI;
use core::fmt::Write as _;

use desk_core::anim::{self, CubicBezier, Curve, MOOOK, PEBBLE_FRAME, PEEK};
use desk_core::app::{Button, DeskApp, Phase, WallTime};
use desk_core::domain::{Agent, MAX_NAME_BYTES, SessionState};
use desk_core::math::{self, lround as px};
use desk_core::scene::{RowSlot, layout};
use desk_core::time::{Instant, Millis, progress};

use crate::canvas::{Canvas, Datum, TextStyle};
use crate::fonts::Fonts;
use crate::glyphs::{self, Face};
use crate::theme::{self, Rgb, mix};

// Row geometry, relative to the row's top-left.
const BAR_X: f32 = 10.0;
const GLYPH_X: f32 = 26.0;
const NAME_X: f32 = 40.0;
const NAME_WIDTH: f32 = 182.0;
const DETAIL_X: f32 = 228.0;
const TIME_RIGHT: f32 = 310.0;

const SPIN_PERIOD: Millis = Millis(1_200);
const HOP_EVERY: Millis = Millis(3_000);

/// A short formatted string; overflowing writes are cut off.
type Line = heapless::String<24>;

fn line(args: core::fmt::Arguments<'_>) -> Line {
    let mut s = Line::new();
    let _ = s.write_fmt(args);
    s
}

/// Fraction of a repeating period, in [0, 1).
fn phase(elapsed: Millis, period: Millis) -> f32 {
    let p = period.count();
    let rem = ((elapsed.count() % p) + p) % p;
    rem as f32 / p as f32
}

/// Vertical offset of the NEEDS YOU glyph's hop: squash, spring up, land with Pebble's peek
/// bounce.
#[derive(Clone, Copy)]
struct Hop {
    y: f32,
    /// Vertical scale.
    squash: f32,
}

fn hop_at(since_state_change: Millis) -> Hop {
    const SQUASH: Millis = PEBBLE_FRAME;
    const RISE: Millis = Millis(120);
    const HEIGHT: f32 = -5.0;
    let t = since_state_change.rem(HOP_EVERY);
    if t < SQUASH {
        return Hop { y: 0.0, squash: 0.75 };
    }
    if t < SQUASH + RISE {
        let k = anim::ease(Curve::EaseOut, progress(t - SQUASH, RISE));
        return Hop { y: HEIGHT * k, squash: 1.0 + 0.12 * (1.0 - k) };
    }
    let fall = t - SQUASH - RISE;
    if fall < PEEK.duration() {
        return Hop { y: anim::moook(&PEEK, HEIGHT, 0.0, fall), squash: 1.0 };
    }
    Hop { y: 0.0, squash: 1.0 }
}

fn format_duration(d: Millis, state: SessionState) -> Line {
    let total = d.count() / 1000;
    if state == SessionState::Done {
        return if total < 60 {
            line(format_args!("now"))
        } else if total < 3600 {
            line(format_args!("{}m", total / 60))
        } else {
            line(format_args!("{}h", total / 3600))
        };
    }
    if total < 3600 {
        line(format_args!("{}:{:02}", total / 60, total % 60))
    } else {
        line(format_args!("{}h{:02}", total / 3600, (total / 60) % 60))
    }
}

/// Bytes of `text` up to its first NUL, as C strings end.
fn c_str(text: &[u8]) -> &[u8] {
    text.iter().position(|&b| b == 0).map_or(text, |end| &text[..end])
}

/// A name cut to fit, with room for the ellipsis.
type Fitted = heapless::Vec<u8, { MAX_NAME_BYTES + 4 }>;

/// Draws one frame of the app into a canvas.
#[derive(Clone, Debug)]
pub struct Renderer {
    fonts: Fonts,
    last_hold: Button,
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new()
    }
}

/// One frame's drawing: the canvas plus what every part needs.
struct Frame<'a, B> {
    g: &'a mut Canvas<B>,
    fonts: &'a Fonts,
    app: &'a DeskApp,
    now: Instant,
}

impl Renderer {
    #[must_use]
    pub fn new() -> Self {
        Self { fonts: Fonts::load(), last_hold: Button::Aux }
    }

    pub fn draw<B: AsRef<[u16]> + AsMut<[u16]>>(
        &mut self,
        app: &DeskApp,
        now: Instant,
        canvas: &mut Canvas<B>,
    ) {
        let stage = app.stage();
        canvas.fill_screen(theme::BACKGROUND.hex());
        let mut f = Frame { g: canvas, fonts: &self.fonts, app, now };

        if stage.pairing() < 0.999 {
            if stage.quiet() < 0.999 && stage.asleep() < 0.999 {
                f.draw_list(&mut self.last_hold);
                f.draw_port_hole(stage.quiet());
            }
            if stage.quiet() > 0.001 && stage.asleep() < 0.5 {
                f.draw_quiet();
            }
            if stage.asleep() > 0.001 {
                f.draw_asleep();
            }
            f.draw_header(app.wall_time(now));
        }
        if stage.pairing() > 0.001 {
            f.draw_pairing();
        }
    }
}

impl<B: AsRef<[u16]> + AsMut<[u16]>> Frame<'_, B> {
    fn text(&mut self, font: &crate::vlw::Font<'_>, text: &[u8], x: i32, y: i32, style: TextStyle) {
        self.g.draw_string(font, text, x, y, style);
    }

    fn draw_list(&mut self, last_hold: &mut Button) {
        let scene = self.app.scene();
        let stage = self.app.stage();
        let origin_y = layout::HEADER_HEIGHT - scene.scroll() + scene.nudge_offset() + stage.wake();
        self.g.set_clip_rect(
            0,
            px(layout::HEADER_HEIGHT),
            px(layout::SCREEN_WIDTH),
            px(layout::VIEWPORT_HEIGHT),
        );

        // Highlight under the selected row, with the long-press fill sweeping across it.
        let highlight =
            scene.highlight_opacity() * (1.0 - stage.offline()) * (1.0 - stage.asleep());
        if highlight > 0.01 {
            let top = px(origin_y + scene.highlight_y()) + 2;
            self.g.fill_smooth_round_rect(
                4,
                top,
                312,
                26,
                8,
                mix(theme::BACKGROUND, theme::SURFACE, highlight).hex(),
            );
            if let Some(hold) = self.app.gestures().hold(self.now) {
                *last_hold = hold.button;
            }
            let fill = scene.hold_fill();
            if fill > 0.005 {
                let tint = if *last_hold == Button::Boot { theme::JUMP } else { theme::DONE };
                let width = 16.max(px(312.0 * fill));
                let color = mix(theme::SURFACE, tint, 0.28 * (fill * 3.0).min(1.0));
                self.g.fill_smooth_round_rect(4, top, width, 26, 8, color.hex());
            }
        }

        // Leaving rows first, so rows arriving or reordering pass over them.
        for slot in scene.slots().iter().filter(|s| s.exiting) {
            self.draw_row(slot);
        }
        for slot in scene.slots().iter().filter(|s| !s.exiting && s.lift.value() < 0.01) {
            self.draw_row(slot);
        }
        for slot in scene.slots().iter().filter(|s| !s.exiting && s.lift.value() >= 0.01) {
            self.draw_row(slot);
        }

        // Scroll hint on the right edge when the list overflows.
        let content = scene.content_height();
        if content > layout::VIEWPORT_HEIGHT {
            let ratio = layout::VIEWPORT_HEIGHT / content;
            let thumb = (layout::VIEWPORT_HEIGHT * ratio).max(12.0);
            let travel = layout::VIEWPORT_HEIGHT - thumb;
            let pos = scene.scroll() / (content - layout::VIEWPORT_HEIGHT);
            self.g.fill_smooth_round_rect(
                316,
                px(layout::HEADER_HEIGHT + travel * pos) + 2,
                2,
                px(thumb) - 4,
                1,
                theme::TEXT_FAINT.hex(),
            );
        }
        self.g.clear_clip_rect();
    }

    #[allow(clippy::too_many_lines)]
    fn draw_row(&mut self, slot: &RowSlot) {
        let (app, now) = (self.app, self.now);
        let scene = app.scene();
        let stage = app.stage();
        let session = &slot.session;
        let origin_y = layout::HEADER_HEIGHT - scene.scroll() + scene.nudge_offset() + stage.wake();

        // Falling asleep: rows gather towards the middle one after another and fade.
        let index = (slot.y.target() / layout::ROW_HEIGHT).max(0.0) as usize;
        let collapse = anim::ease(Curve::EaseIn, stage.collapse(index));
        let middle = scene.scroll() + layout::VIEWPORT_HEIGHT / 2.0 - layout::ROW_HEIGHT / 2.0;
        let top = origin_y + slot.y.value() + (middle - slot.y.value()) * collapse;
        if top > layout::SCREEN_HEIGHT || top + layout::ROW_HEIGHT < layout::HEADER_HEIGHT {
            return;
        }
        let left = slot.x.value();
        let center_y = top + layout::ROW_HEIGHT / 2.0;
        let opacity =
            slot.opacity.value().clamp(0.0, 1.0) * (1.0 - 0.5 * stage.offline()) * (1.0 - collapse);
        let scale = slot.scale.value();
        let selected = scene.selected_id() == Some(session.id) && !slot.exiting;
        let fade = |c: Rgb| mix(theme::BACKGROUND, c, opacity).hex();

        // Lift: a brighter plate while held or hopping to a new place.
        let lift = slot.lift.value().clamp(0.0, 1.0);
        if lift > 0.01 {
            let base = if selected { theme::SURFACE } else { theme::BACKGROUND };
            self.g.fill_smooth_round_rect(
                px(left + 4.0),
                px(top + 2.0),
                312,
                26,
                8,
                fade(mix(base, theme::LIFTED, lift)),
            );
        }

        // State bar: crossfades between states; NEEDS YOU breathes.
        let mut bar = mix(
            theme::state_color(slot.previous_state),
            theme::state_color(session.state),
            slot.state_blend.value(),
        );
        if session.state == SessionState::NeedsYou {
            bar =
                mix(mix(theme::BACKGROUND, bar, 0.45), bar, anim::breathe(now - slot.state_since));
        }
        let bar_height = 16.0 * scale;
        self.g.fill_smooth_round_rect(
            px(left + BAR_X),
            px(center_y - bar_height / 2.0),
            4,
            px(bar_height),
            2,
            fade(bar),
        );

        // Glyph: spins while working, hops when it needs you, pops into a check when done.
        let in_state = now - slot.state_since;
        let agent_color = fade(theme::agent_color(session.agent));
        let glyph_x = left + GLYPH_X;
        if session.state == SessionState::Done {
            let r = 6.5 * slot.glyph_scale.value().max(0.0) * scale;
            glyphs::check(self.g, glyph_x, center_y, r, fade(theme::DONE));
        } else {
            let mut angle = 0.0;
            let mut glyph_y = center_y;
            let mut r = 6.0 * scale;
            if session.state == SessionState::Working {
                angle = 2.0 * PI * phase(now.since_boot, SPIN_PERIOD);
            } else {
                let hop = hop_at(in_state);
                glyph_y += hop.y + r * (1.0 - hop.squash);
                r *= hop.squash;
            }
            match session.agent {
                Agent::Claude => glyphs::sparkle(self.g, glyph_x, glyph_y, r, angle, agent_color),
                Agent::Codex => {
                    glyphs::hexagon(self.g, glyph_x, glyph_y, r * 0.85, angle / 2.0, agent_color);
                }
            }
        }

        // Name: ellipsised, or scrolling through when selected and too long.
        let fonts = self.fonts;
        let name = c_str(session.name.as_bytes());
        let name_style = TextStyle::new(Datum::MiddleLeft, fade(theme::TEXT)).sized(scale);
        let name_width = self.g.text_width(&fonts.name, name, scale) as f32;
        if selected && name_width > NAME_WIDTH {
            let offset =
                anim::marquee_offset(name_width - NAME_WIDTH, now - scene.selected_since());
            let clip_top = px(top).max(px(layout::HEADER_HEIGHT));
            self.g.set_clip_rect(
                px(left + NAME_X),
                clip_top,
                px(NAME_WIDTH),
                px(layout::ROW_HEIGHT),
            );
            self.text(&fonts.name, name, px(left + NAME_X - offset), px(center_y), name_style);
            self.g.set_clip_rect(
                0,
                px(layout::HEADER_HEIGHT),
                px(layout::SCREEN_WIDTH),
                px(layout::VIEWPORT_HEIGHT),
            );
        } else {
            let fitted = self.fit_text(name, NAME_WIDTH, scale);
            self.text(&fonts.name, &fitted, px(left + NAME_X), px(center_y), name_style);
        }

        // Detail and time.
        let detail_style = TextStyle::new(Datum::MiddleLeft, fade(theme::TEXT_DIM)).sized(scale);
        if session.subagents > 0 {
            let detail = line(format_args!("×{}", session.subagents));
            self.text(
                &fonts.small,
                detail.as_bytes(),
                px(left + DETAIL_X),
                px(center_y),
                detail_style,
            );
        } else {
            let detail = c_str(session.detail.as_bytes());
            self.text(&fonts.small, detail, px(left + DETAIL_X), px(center_y), detail_style);
        }

        let time = format_duration(app.board().time_in_state(session, now), session.state);
        let time_style = TextStyle::new(Datum::MiddleRight, fade(theme::TEXT_FAINT)).sized(scale);
        self.text(&fonts.small, time.as_bytes(), px(left + TIME_RIGHT), px(center_y), time_style);
    }

    /// Cuts `text` to fit `width` px, ending with an ellipsis when shortened.
    fn fit_text(&mut self, text: &[u8], width: f32, size: f32) -> Fitted {
        let font = &self.fonts.name;
        let mut out = Fitted::new();
        let _ = out.extend_from_slice(&text[..text.len().min(out.capacity() - 1)]);
        if self.g.text_width(font, &out, size) as f32 <= width {
            return out;
        }
        let ellipsis = "…".as_bytes();
        let mut len = out.len();
        while len > 0 {
            loop {
                len -= 1; // step back over a whole UTF-8 sequence
                if !(len > 0 && out[len] & 0xC0 == 0x80) {
                    break;
                }
            }
            while len > 0 && out[len - 1] == b' ' {
                len -= 1;
            }
            out.truncate(len);
            let _ = out.extend_from_slice(ellipsis);
            if self.g.text_width(font, &out, size) as f32 <= width {
                return out;
            }
        }
        out
    }

    fn draw_header(&mut self, wall_time: Option<WallTime>) {
        let scene = self.app.scene();
        self.g.fill_rect(
            0,
            0,
            px(layout::SCREEN_WIDTH),
            px(layout::HEADER_HEIGHT),
            theme::BACKGROUND.hex(),
        );
        if scene.scroll() > 0.5 && self.app.stage().quiet() < 0.5 {
            self.g.draw_fast_hline(
                0,
                px(layout::HEADER_HEIGHT) - 1,
                px(layout::SCREEN_WIDTH),
                theme::HAIRLINE.hex(),
            );
        }

        // Counters lift away and fade as the "Mac asleep" status rises in to replace them.
        const TRAVEL: f32 = 7.0;
        let away = anim::ease(Curve::EaseInOut, self.app.stage().offline());
        if away < 0.5 {
            self.draw_counters(-away * 2.0 * TRAVEL, 1.0 - away * 2.0);
        } else {
            self.draw_away_status((1.0 - away) * 2.0 * TRAVEL, (away - 0.5) * 2.0);
        }

        if let Some(t) = wall_time {
            let clock = line(format_args!("{:02}:{:02}", t.hour, t.minute));
            let style = TextStyle::new(Datum::MiddleRight, theme::TEXT_DIM.hex());
            let small = &self.fonts.small;
            self.text(
                small,
                clock.as_bytes(),
                px(layout::SCREEN_WIDTH) - 8,
                px(layout::HEADER_HEIGHT / 2.0),
                style,
            );
        }
    }

    fn draw_counters(&mut self, offset_y: f32, alpha: f32) {
        let scene = self.app.scene();
        let font = &self.fonts.label;
        let mid_y = layout::HEADER_HEIGHT / 2.0 + offset_y;
        let line_height = 14.0;
        let faded = |c: Rgb| mix(theme::BACKGROUND, c, alpha).hex();

        let mut x = 8.0;
        for (state, label) in [
            (SessionState::NeedsYou, "NEED"),
            (SessionState::Done, "DONE"),
            (SessionState::Working, "WORK"),
        ] {
            let value = scene.count(state).max(0.0);
            let any = value > 0.05;
            let dot = if any { theme::state_color(state) } else { theme::TEXT_FAINT };
            self.g.dot(x + 3.0, mid_y, 3.0, faded(dot));

            // Odometer: the old number rolls up and out as the new one rolls in.
            let whole = math::floor(value);
            let frac = value - whole;
            let digit_x = px(x + 11.0);
            self.g.set_clip_rect(digit_x, 2, 18, px(layout::HEADER_HEIGHT) - 4);
            let digit_style = TextStyle::new(
                Datum::MiddleLeft,
                faded(if any { theme::TEXT } else { theme::TEXT_FAINT }),
            );
            let mut digits = line(format_args!("{}", whole as i32));
            self.text(
                font,
                digits.as_bytes(),
                digit_x,
                px(mid_y - frac * line_height),
                digit_style,
            );
            if frac > 0.01 {
                digits = line(format_args!("{}", whole as i32 + 1));
                self.text(
                    font,
                    digits.as_bytes(),
                    digit_x,
                    px(mid_y + (1.0 - frac) * line_height),
                    digit_style,
                );
            }
            self.g.clear_clip_rect();

            let label_style = TextStyle::new(Datum::MiddleLeft, faded(theme::TEXT_DIM));
            let label_x = digit_x + 8.max(self.g.text_width(font, digits.as_bytes(), 1.0)) + 3;
            self.text(font, label.as_bytes(), label_x, px(mid_y), label_style);
            x = (label_x + self.g.text_width(font, label.as_bytes(), 1.0)) as f32 + 12.0;
        }
    }

    fn draw_away_status(&mut self, offset_y: f32, alpha: f32) {
        let mid_y = layout::HEADER_HEIGHT / 2.0 + offset_y;
        self.g.dot(11.0, mid_y, 3.0, mix(theme::BACKGROUND, theme::TEXT_FAINT, alpha).hex());
        let small = &self.fonts.small;
        let style =
            TextStyle::new(Datum::MiddleLeft, mix(theme::BACKGROUND, theme::TEXT_DIM, alpha).hex());
        let label = b"Mac asleep";
        self.text(small, label, 19, px(mid_y), style);
        let after = (19 + self.g.text_width(small, label, 1.0)) as f32 + 6.0;
        self.draw_zzz(after, mid_y + 2.0, alpha);
    }

    fn draw_zzz(&mut self, x: f32, y: f32, strength: f32) {
        // Three z's drifting up and to the right, each fading in and out on its own beat.
        let small = &self.fonts.small;
        for k in 0..3 {
            let p = phase(self.now.since_boot + Millis(k * 800), Millis(2_400));
            let alpha = math::sin(PI * p) * strength;
            let style = TextStyle::new(
                Datum::MiddleCenter,
                mix(theme::BACKGROUND, theme::TEXT_DIM, alpha).hex(),
            )
            .sized(0.7 + 0.4 * p);
            self.text(small, b"z", px(x + 7.0 * p), px(y - 9.0 * p), style);
        }
    }

    fn draw_port_hole(&mut self, progress: f32) {
        // Pebble's port hole: the list shrinks away through a closing circle.
        if progress <= 0.001 {
            return;
        }
        const CX: f32 = layout::SCREEN_WIDTH / 2.0;
        const CY: f32 = layout::HEADER_HEIGHT + layout::VIEWPORT_HEIGHT / 2.0;
        const FAR: f32 = 190.0;
        let radius = FAR * (1.0 - anim::ease(Curve::EaseInOut, progress));
        self.g.fill_arc(
            px(CX),
            px(CY),
            px(radius).max(0),
            px(FAR),
            0.0,
            360.0,
            theme::BACKGROUND.hex(),
        );
    }

    fn draw_quiet(&mut self) {
        let stage = self.app.stage();
        let now = self.now.since_boot;
        const CX: f32 = layout::SCREEN_WIDTH / 2.0;
        const CY: f32 = 82.0;
        // Grows out of the port hole with a little overshoot.
        const BACK_OUT: CubicBezier = CubicBezier::new(0.34, 1.56, 0.64, 1.0);
        let grow = BACK_OUT.at(stage.quiet());
        let radius = 20.0 * grow;
        if radius < 0.5 {
            return;
        }

        // Idle life: a slow breath, a blink every few seconds; sleepier while the Mac is away.
        let breath = math::sin(2.0 * PI * phase(now, Millis(3_200)));
        let blink_phase = phase(now, Millis(4_200));
        let blink = if blink_phase > 0.95 {
            math::cos(PI * (blink_phase - 0.95) / 0.05).abs()
        } else {
            1.0
        };
        let face = Face {
            eyes_open: blink * (1.0 - 0.55 * stage.offline()),
            smile: 1.0 - 0.6 * stage.offline(),
            squash: 1.0 + 0.035 * breath,
        };
        glyphs::face(self.g, CX, CY, radius, &face, theme::SURFACE.hex(), theme::TEXT_DIM.hex());

        let style = TextStyle::new(
            Datum::MiddleCenter,
            mix(theme::BACKGROUND, theme::TEXT_DIM, stage.quiet()).hex(),
        );
        let name = &self.fonts.name;
        self.text(name, b"All quiet", px(CX), px(CY + 40.0 + 4.0 * (1.0 - stage.quiet())), style);
        if stage.offline() > 0.01 {
            self.draw_zzz(CX + radius + 6.0, CY - radius * 0.6, stage.offline());
        }
    }

    fn draw_asleep(&mut self) {
        let stage = self.app.stage();
        let appear = ((stage.asleep() - 0.5) * 2.0).clamp(0.0, 1.0);
        if appear <= 0.0 {
            return;
        }
        const CX: f32 = layout::SCREEN_WIDTH / 2.0;
        const CY: f32 = 86.0;
        let breath = math::sin(2.0 * PI * phase(self.now.since_boot, Millis(4_000)));
        let face = Face { eyes_open: 0.0, smile: 0.35, squash: 1.0 + 0.05 * breath };
        let radius = 20.0 * anim::ease(Curve::EaseOut, appear);
        glyphs::face(
            self.g,
            CX,
            CY,
            radius,
            &face,
            mix(theme::BACKGROUND, theme::SURFACE, appear).hex(),
            mix(theme::BACKGROUND, theme::TEXT_FAINT, appear).hex(),
        );
        self.draw_zzz(CX + radius + 6.0, CY - radius * 0.6, appear);
    }

    fn draw_pairing(&mut self) {
        let (app, now) = (self.app, self.now);
        let stage = app.stage();
        let fonts = self.fonts;
        let x = (1.0 - stage.pairing()) * layout::SCREEN_WIDTH;
        self.g.fill_rect(
            px(x),
            0,
            px(layout::SCREEN_WIDTH) + 8,
            px(layout::SCREEN_HEIGHT),
            theme::BACKGROUND.hex(),
        );
        self.g.draw_fast_vline(px(x), 0, px(layout::SCREEN_HEIGHT), theme::HAIRLINE.hex());

        let blink_phase = phase(now.since_boot, Millis(3_600));
        let face = Face {
            eyes_open: if blink_phase > 0.95 { 0.1 } else { 1.0 },
            squash: 1.0 + 0.03 * math::sin(2.0 * PI * phase(now.since_boot, Millis(2_800))),
            ..Face::default()
        };

        if app.phase() == Phase::Pairing {
            let centered = |c: Rgb| TextStyle::new(Datum::MiddleCenter, c.hex());
            self.text(
                &fonts.small,
                b"Confirm this code on your Mac",
                px(x + 160.0),
                30,
                centered(theme::TEXT_DIM),
            );

            // Digits drop in one after another with a moook.
            let code = line(format_args!("{:06}", app.connection().passkey()));
            let since = now - stage.pairing_since();
            let mut digit_x = x + 160.0 - 3.0 * 30.0 - 8.0;
            for (i, digit) in code.as_bytes().iter().take(6).enumerate() {
                let local = (since - Millis(i as i64 * 60)).max(Millis::ZERO);
                let dy = anim::moook(&MOOOK, -24.0, 0.0, local);
                let alpha = progress(local, Millis(120));
                let style = centered(mix(theme::BACKGROUND, theme::TEXT, alpha));
                self.text(&fonts.digits, &[*digit], px(digit_x + 15.0), px(92.0 + dy), style);
                digit_x += 30.0 + if i == 2 { 16.0 } else { 0.0 };
            }
            let hint = "Hold ↑ + ↓ for 5 s to start over".as_bytes();
            self.text(&fonts.small, hint, px(x + 160.0), 150, centered(theme::TEXT_FAINT));
            return;
        }

        glyphs::face(
            self.g,
            x + 62.0,
            78.0,
            22.0,
            &face,
            theme::SURFACE.hex(),
            theme::TEXT_DIM.hex(),
        );
        let left = |c: Rgb| TextStyle::new(Datum::MiddleLeft, c.hex());
        self.text(&fonts.name, b"Let's pair", px(x + 112.0), 62, left(theme::TEXT));
        self.text(
            &fonts.small,
            b"On your Mac: desksessions",
            px(x + 112.0),
            86,
            left(theme::TEXT_DIM),
        );
        self.text(
            &fonts.small,
            "→ Pair New Clock".as_bytes(),
            px(x + 112.0),
            102,
            left(theme::TEXT_DIM),
        );
    }
}

/// The colour a canvas pixel ends up as after a backlight at `level` dims it: how the simulator
/// shows brightness, since a window has no backlight.
#[must_use]
pub fn dim_for_backlight(pixel: u16, level: f32) -> u16 {
    if level >= 0.999 {
        return pixel;
    }
    // Perceived brightness: a dimmed backlight still shows something at the bottom end.
    let scale = (256.0 * (0.08 + 0.92 * level)) as u32;
    let r = (((u32::from(pixel) >> 11) & 0x1F) * scale) >> 8;
    let g = (((u32::from(pixel) >> 5) & 0x3F) * scale) >> 8;
    let b = ((u32::from(pixel) & 0x1F) * scale) >> 8;
    ((r << 11) | (g << 5) | b) as u16
}
