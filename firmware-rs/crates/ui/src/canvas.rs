//! An RGB565 frame buffer with the drawing primitives the renderer uses.
//!
//! The rasterisers follow LovyanGFX 1.2 (which the C++ firmware drew with) step for step:
//! anti-aliased wedge lines and rounded rectangles, arcs, clipping, alpha blending through
//! 8-bit channels, and VLW text with fractional scaling. Matching them is what keeps the pictures
//! the same; where they have quirks (a wedge line resets the clip rectangle when it is done) the
//! quirks are kept too.

use desk_core::math;

use crate::vlw::Font;

/// 0xRRGGBB.
pub type Rgb888 = u32;

const LO_ALPHA: f32 = 1.0 / 32.0;
const HI_ALPHA: f32 = 1.0 - LO_ALPHA;
const DEG_TO_RAD: f32 = 0.017_453_292;

/// Packs 0xRRGGBB into RGB565 by truncation.
#[must_use]
pub const fn rgb565(rgb: Rgb888) -> u16 {
    let r = (rgb >> 16) & 0xFF;
    let g = (rgb >> 8) & 0xFF;
    let b = rgb & 0xFF;
    ((r >> 3) << 11 | (g >> 2) << 5 | (b >> 3)) as u16
}

/// Widens RGB565 back to 8-bit channels, replicating the high bits into the low ones.
#[must_use]
pub const fn expand565(c: u16) -> (u8, u8, u8) {
    let r5 = ((c >> 11) & 0x1F) as u8;
    let g6 = ((c >> 5) & 0x3F) as u8;
    let b5 = (c & 0x1F) as u8;
    ((r5 << 3) | (r5 >> 2), (g6 << 2) | (g6 >> 4), (b5 << 3) | (b5 >> 2))
}

const fn rgb888(r: u8, g: u8, b: u8) -> Rgb888 {
    (r as u32) << 16 | (g as u32) << 8 | b as u32
}

/// Where text is anchored relative to the point it is drawn at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Datum {
    TopLeft,
    MiddleLeft,
    MiddleCenter,
    MiddleRight,
}

impl Datum {
    const fn middle(self) -> bool {
        !matches!(self, Self::TopLeft)
    }
}

/// How text is drawn: anchor, scale and colour.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextStyle {
    pub datum: Datum,
    pub size: f32,
    pub color: Rgb888,
}

impl TextStyle {
    #[must_use]
    pub const fn new(datum: Datum, color: Rgb888) -> Self {
        Self { datum, size: 1.0, color }
    }

    #[must_use]
    pub const fn sized(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    /// LovyanGFX's 16.16 fixed-point scale; sizes ≤ 0 mean 1.
    fn scale(&self) -> i32 {
        let size = if self.size > 0.0 { self.size } else { 1.0 };
        (65536.0 * size) as i32
    }
}

/// LovyanGFX's incremental UTF-8 decoder. Its state outlives a string, as it did there.
#[derive(Clone, Copy, Debug, Default)]
struct Utf8 {
    pending: u8,
    code: u32,
}

impl Utf8 {
    fn feed(&mut self, c: u8) -> u32 {
        if c & 0x80 != 0 {
            if self.pending > 0 {
                self.pending -= 1;
                self.code |= u32::from(c & 0x3F) << (6 * u32::from(self.pending));
                return if self.pending > 0 { 0 } else { self.code };
            }
            if c & 0xE0 == 0xC0 {
                self.code = u32::from(c & 0x1F) << 6;
                self.pending = 1;
                return 0;
            }
            if c & 0xF0 == 0xE0 {
                self.code = u32::from(c & 0x0F) << 12;
                self.pending = 2;
                return 0;
            }
            if c & 0xF8 == 0xF0 {
                self.code = u32::from(c & 0x07) << 18;
                self.pending = 3;
                return 0;
            }
        }
        // Single byte, or a stray byte shown as extended ASCII.
        self.pending = 0;
        u32::from(c)
    }
}

/// An RGB565 frame buffer of `width × height` logical pixels, optionally rotated in storage.
///
/// `B` owns or borrows the pixels: a `Vec<u16>` in the simulator, a static DMA buffer on the
/// device.
pub struct Canvas<B> {
    pixels: B,
    /// Storage dimensions, before rotation.
    panel_width: i32,
    panel_height: i32,
    width: i32,
    height: i32,
    rotation: u8,
    /// Pixels are stored byte-swapped (big-endian), the order the panel's SPI bus wants.
    swapped: bool,
    clip_l: i32,
    clip_r: i32,
    clip_t: i32,
    clip_b: i32,
    utf8: Utf8,
}

impl<B: AsRef<[u16]> + AsMut<[u16]>> Canvas<B> {
    /// A canvas over `pixels`, `panel_width × panel_height` in storage order.
    ///
    /// # Panics
    ///
    /// If `pixels` is smaller than the panel.
    pub fn new(pixels: B, panel_width: i32, panel_height: i32) -> Self {
        assert!(pixels.as_ref().len() >= (panel_width * panel_height) as usize);
        let mut canvas = Self {
            pixels,
            panel_width,
            panel_height,
            width: panel_width,
            height: panel_height,
            rotation: 0,
            swapped: false,
            clip_l: 0,
            clip_r: 0,
            clip_t: 0,
            clip_b: 0,
            utf8: Utf8::default(),
        };
        canvas.clear_clip_rect();
        canvas
    }

    /// Stores pixels big-endian, as the panel's SPI transfer wants them.
    #[must_use]
    pub fn byte_swapped(mut self) -> Self {
        self.swapped = true;
        self
    }

    /// Turns the logical picture in steps of 90° relative to storage (LovyanGFX's numbering).
    pub fn set_rotation(&mut self, rotation: u8) {
        self.rotation = rotation & 3;
        if self.rotation & 1 == 0 {
            self.width = self.panel_width;
            self.height = self.panel_height;
        } else {
            self.width = self.panel_height;
            self.height = self.panel_width;
        }
        self.clear_clip_rect();
    }

    #[must_use]
    pub fn width(&self) -> i32 {
        self.width
    }

    #[must_use]
    pub fn height(&self) -> i32 {
        self.height
    }

    /// The pixels in storage order.
    #[must_use]
    pub fn pixels(&self) -> &[u16] {
        self.pixels.as_ref()
    }

    pub fn pixels_mut(&mut self) -> &mut [u16] {
        self.pixels.as_mut()
    }

    // ---------------------------------------------------------------------------------------
    // Pixel access

    fn index(&self, x: i32, y: i32) -> usize {
        let (mut x, mut y) = (x, y);
        let r = self.rotation;
        if r != 0 {
            if (1u8 << r) & 0b1001_0110 != 0 {
                y = self.height - (y + 1);
            }
            if r & 2 != 0 {
                x = self.width - (x + 1);
            }
            if r & 1 != 0 {
                core::mem::swap(&mut x, &mut y);
            }
        }
        (x + y * self.panel_width) as usize
    }

    fn store(&mut self, x: i32, y: i32, color: u16) {
        let i = self.index(x, y);
        self.pixels.as_mut()[i] = if self.swapped { color.swap_bytes() } else { color };
    }

    /// The RGB565 value at logical `(x, y)`.
    #[must_use]
    pub fn get(&self, x: i32, y: i32) -> u16 {
        let raw = self.pixels.as_ref()[self.index(x, y)];
        if self.swapped { raw.swap_bytes() } else { raw }
    }

    /// The colour at logical `(x, y)`, widened to 8-bit channels.
    #[must_use]
    pub fn get_rgb(&self, x: i32, y: i32) -> (u8, u8, u8) {
        expand565(self.get(x, y))
    }

    fn fill_preclipped(&mut self, x: i32, y: i32, w: i32, h: i32, color: u16) {
        // Rotating or flipping a rectangle gives a rectangle: fill it row by row in storage.
        let a = self.index(x, y);
        let b = self.index(x + w - 1, y + h - 1);
        let pw = self.panel_width as usize;
        let (ax, ay, bx, by) = (a % pw, a / pw, b % pw, b / pw);
        let (x0, x1) = (ax.min(bx), ax.max(bx));
        let (y0, y1) = (ay.min(by), ay.max(by));
        let raw = if self.swapped { color.swap_bytes() } else { color };
        let pixels = self.pixels.as_mut();
        for row in y0..=y1 {
            pixels[row * pw + x0..=row * pw + x1].fill(raw);
        }
    }

    // ---------------------------------------------------------------------------------------
    // Clipping

    pub fn set_clip_rect(&mut self, x: i32, y: i32, w: i32, h: i32) {
        let (mut x, mut y, mut w, mut h) = (x, y, w, h);
        if x < 0 {
            w += x;
            x = 0;
        }
        if w > self.width - x {
            w = self.width - x;
        }
        if w < 1 {
            x = 0;
            w = 0;
        }
        self.clip_l = x;
        self.clip_r = x + w - 1;

        if y < 0 {
            h += y;
            y = 0;
        }
        if h > self.height - y {
            h = self.height - y;
        }
        if h < 1 {
            y = 0;
            h = 0;
        }
        self.clip_t = y;
        self.clip_b = y + h - 1;
    }

    pub fn clear_clip_rect(&mut self) {
        self.clip_l = 0;
        self.clip_r = self.width - 1;
        self.clip_t = 0;
        self.clip_b = self.height - 1;
    }

    /// Clips a rectangle in place; false when nothing is left.
    fn clip(&self, x: &mut i32, y: &mut i32, w: &mut i32, h: &mut i32) -> bool {
        if *x < self.clip_l {
            *w += *x - self.clip_l;
            *x = self.clip_l;
        }
        *w = (*w).min(self.clip_r + 1 - *x);
        if *w < 1 {
            return false;
        }
        if *y < self.clip_t {
            *h += *y - self.clip_t;
            *y = self.clip_t;
        }
        *h = (*h).min(self.clip_b + 1 - *y);
        *h >= 1
    }

    fn in_clip(&self, x: i32, y: i32) -> bool {
        x >= self.clip_l && x <= self.clip_r && y >= self.clip_t && y <= self.clip_b
    }

    // ---------------------------------------------------------------------------------------
    // Solid shapes

    fn write_fill_rect(&mut self, mut x: i32, mut y: i32, mut w: i32, mut h: i32, color: u16) {
        if self.clip(&mut x, &mut y, &mut w, &mut h) {
            self.fill_preclipped(x, y, w, h, color);
        }
    }

    fn write_fast_hline(&mut self, mut x: i32, y: i32, mut w: i32, color: u16) {
        if y < self.clip_t || y > self.clip_b {
            return;
        }
        if x < self.clip_l {
            w += x - self.clip_l;
            x = self.clip_l;
        }
        w = w.min(self.clip_r + 1 - x);
        if w < 1 {
            return;
        }
        self.fill_preclipped(x, y, w, 1, color);
    }

    fn write_fast_vline(&mut self, x: i32, mut y: i32, mut h: i32, color: u16) {
        if x < self.clip_l || x > self.clip_r {
            return;
        }
        if y < self.clip_t {
            h += y - self.clip_t;
            y = self.clip_t;
        }
        h = h.min(self.clip_b + 1 - y);
        if h < 1 {
            return;
        }
        self.fill_preclipped(x, y, 1, h, color);
    }

    fn write_pixel(&mut self, x: i32, y: i32, color: u16) {
        if self.in_clip(x, y) {
            self.store(x, y, color);
        }
    }

    pub fn fill_screen(&mut self, color: Rgb888) {
        self.fill_rect(0, 0, self.width, self.height, color);
    }

    pub fn fill_rect(&mut self, mut x: i32, mut y: i32, mut w: i32, mut h: i32, color: Rgb888) {
        if w < 0 {
            x += w;
            w = -w;
        }
        if h < 0 {
            y += h;
            h = -h;
        }
        self.write_fill_rect(x, y, w, h, rgb565(color));
    }

    pub fn draw_fast_hline(&mut self, mut x: i32, y: i32, mut w: i32, color: Rgb888) {
        if w < 0 {
            x += w;
            w = -w;
        }
        self.write_fast_hline(x, y, w, rgb565(color));
    }

    pub fn draw_fast_vline(&mut self, x: i32, mut y: i32, mut h: i32, color: Rgb888) {
        if h < 0 {
            y += h;
            h = -h;
        }
        self.write_fast_vline(x, y, h, rgb565(color));
    }

    pub fn draw_pixel(&mut self, x: i32, y: i32, color: Rgb888) {
        self.write_pixel(x, y, rgb565(color));
    }

    /// One-pixel outline.
    pub fn draw_rect(&mut self, mut x: i32, mut y: i32, mut w: i32, mut h: i32, color: Rgb888) {
        if w < 0 {
            x += w;
            w = -w;
        }
        if h < 0 {
            y += h;
            h = -h;
        }
        if w == 0 || h == 0 {
            return;
        }
        let c = rgb565(color);
        self.write_fast_hline(x, y, w, c);
        h -= 1;
        if h != 0 {
            self.write_fast_hline(x, y + h, w, c);
            h -= 1;
            if h != 0 {
                y += 1;
                self.write_fast_vline(x, y, h, c);
                self.write_fast_vline(x + w - 1, y, h, c);
            }
        }
    }

    // ---------------------------------------------------------------------------------------
    // Blending

    /// Blends `color` over a rectangle at `alpha` / 255.
    pub fn fill_rect_alpha(
        &mut self,
        mut x: i32,
        mut y: i32,
        mut w: i32,
        mut h: i32,
        alpha: u8,
        color: Rgb888,
    ) {
        if !self.clip(&mut x, &mut y, &mut w, &mut h) {
            return;
        }
        let a = u32::from(alpha);
        let inv = 256 - a;
        let r = ((color >> 16) & 0xFF) * (1 + a);
        let g = ((color >> 8) & 0xFF) * (1 + a);
        let b = (color & 0xFF) * (1 + a);
        for row in y..y + h {
            for col in x..x + w {
                let (dr, dg, db) = self.get_rgb(col, row);
                let out = rgb888(
                    ((r + u32::from(dr) * inv) >> 8) as u8,
                    ((g + u32::from(dg) * inv) >> 8) as u8,
                    ((b + u32::from(db) * inv) >> 8) as u8,
                );
                self.store(col, row, rgb565(out));
            }
        }
    }

    // ---------------------------------------------------------------------------------------
    // Anti-aliased shapes

    /// Filled rectangle with anti-aliased corners of radius `r`.
    pub fn fill_smooth_round_rect(
        &mut self,
        mut x: i32,
        mut y: i32,
        mut w: i32,
        mut h: i32,
        mut r: i32,
        color: Rgb888,
    ) {
        let raw = rgb565(color);
        let (er, eg, eb) = expand565(raw);
        let edge = rgb888(er, eg, eb);
        let mut xs = 0;
        // Limit radius to half width or height.
        r = r.min(w / 2).min(h / 2);

        y += r;
        h -= 2 * r;
        self.fill_rect(x, y, w, h, edge);
        h -= 1;
        x += r;
        w -= 2 * r + 1;
        let r1 = r * r;
        r += 1;
        let r2 = r * r;

        let mut cy = r - 1;
        while cy > 0 {
            let dy2 = (r - cy) * (r - cy);
            let mut cx = xs;
            while cx < r {
                let hyp2 = (r - cx) * (r - cx) + dy2;
                if hyp2 <= r1 {
                    break;
                }
                if hyp2 >= r2 {
                    cx += 1;
                    continue;
                }
                let alphaf = r as f32 - math::sqrt(hyp2 as f32);
                if alphaf > HI_ALPHA {
                    break;
                }
                xs = cx;
                if alphaf < LO_ALPHA {
                    cx += 1;
                    continue;
                }
                let alpha = (alphaf * 255.0) as u8;
                self.fill_rect_alpha(x + cx - r, y + cy - r, 1, 1, alpha, edge);
                self.fill_rect_alpha(x - cx + r + w, y + cy - r, 1, 1, alpha, edge);
                self.fill_rect_alpha(x - cx + r + w, y - cy + r + h, 1, 1, alpha, edge);
                self.fill_rect_alpha(x + cx - r, y - cy + r + h, 1, 1, alpha, edge);
                cx += 1;
            }
            self.write_fast_hline(x + cx - r, y + cy - r, 2 * (r - cx) + 1 + w, raw);
            self.write_fast_hline(x + cx - r, y - cy + r + h, 2 * (r - cx) + 1 + w, raw);
            cy -= 1;
        }
    }

    /// Shrinks a bounding box to the canvas; false when it misses entirely.
    fn clamp_area(&self, xlo: &mut i32, ylo: &mut i32, xhi: &mut i32, yhi: &mut i32) -> bool {
        if *xlo >= self.width || *ylo >= self.height || *xhi < 0 || *yhi < 0 {
            return false;
        }
        *xlo = (*xlo).max(0);
        *ylo = (*ylo).max(0);
        if *xhi > self.width {
            *xhi = self.width - 1;
        }
        if *yhi > self.height {
            *yhi = self.height - 1;
        }
        true
    }

    /// An anti-aliased line from `a` to `b` whose radius tapers from `ar` to `br`, with round
    /// ends. Sub-pixel exact, so small spinning glyphs don't shimmer.
    ///
    /// Like LovyanGFX's, it replaces the clip rectangle with its own bounding box and clears it
    /// afterwards.
    #[allow(clippy::too_many_arguments)]
    pub fn wedge(
        &mut self,
        ax: f32,
        ay: f32,
        mut bx: f32,
        by: f32,
        ar: f32,
        br: f32,
        color: Rgb888,
    ) {
        if ar < 0.0 || br < 0.0 {
            return;
        }
        if (ax - bx).abs() < 0.01 && (ay - by).abs() < 0.01 {
            bx += 0.01; // avoid dividing by zero
        }
        let mut x0 = math::floor((ax - ar).min(bx - br)) as i32;
        let mut x1 = math::ceil((ax + ar).max(bx + br)) as i32;
        let mut y0 = math::floor((ay - ar).min(by - br)) as i32;
        let mut y1 = math::ceil((ay + ar).max(by + br)) as i32;
        if !self.clamp_area(&mut x0, &mut y0, &mut x1, &mut y1) {
            return;
        }
        self.set_clip_rect(x0, y0, x1 - x0 + 1, y1 - y0 + 1);

        let solid = rgb565(color);
        let mut ys = ay as i32;
        if (ax - ar) > (bx - br) {
            ys = by as i32;
        }
        let rdt = ar - br;
        let ar = ar + 0.5; // centre of the pixel
        let bax = bx - ax;
        let bay = by - ay;
        // Carried from pixel to pixel and row to row, exactly as the original scan does.
        let mut alpha = 1.0_f32;

        let mut scan = |canvas: &mut Self, yp: i32, xs: &mut i32| {
            let mut end_x = false;
            let ypay = yp as f32 - ay;
            let mut xp = *xs;
            while xp <= x1 {
                if end_x && alpha <= LO_ALPHA {
                    break; // past the line's right edge
                }
                let xpax = xp as f32 - ax;
                alpha = ar - wedge_line_distance(xpax, ypay, bax, bay, rdt);
                if alpha <= LO_ALPHA {
                    xp += 1;
                    continue;
                }
                if !end_x {
                    end_x = true;
                    *xs = xp;
                }
                if alpha > HI_ALPHA {
                    canvas.write_pixel(xp, yp, solid);
                } else {
                    canvas.fill_rect_alpha(xp, yp, 1, 1, (alpha * 255.0) as u8, color);
                }
                xp += 1;
            }
        };

        // From the start row down, then from the row above it up.
        let mut xs = x0;
        for yp in ys..=y1 {
            scan(self, yp, &mut xs);
        }
        let mut xs = x0;
        let mut yp = ys - 1;
        while yp >= y0 {
            scan(self, yp, &mut xs);
            yp -= 1;
        }
        self.clear_clip_rect();
    }

    /// A round-ended line of constant `radius`.
    pub fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, radius: f32, color: Rgb888) {
        self.wedge(x0, y0, x1, y1, radius, radius, color);
    }

    /// An anti-aliased dot.
    pub fn dot(&mut self, x: f32, y: f32, radius: f32, color: Rgb888) {
        self.wedge(x, y, x, y, radius, radius, color);
    }

    /// A filled ring between radii `r0` and `r1`, from `angle0` to `angle1` degrees.
    #[allow(clippy::too_many_arguments)]
    pub fn fill_arc(
        &mut self,
        x: i32,
        y: i32,
        r0: i32,
        r1: i32,
        angle0: f32,
        angle1: f32,
        color: Rgb888,
    ) {
        let (mut r0x, mut r1x, mut r0y, mut r1y) = (r0, r1, r0, r1);
        if r0x < r1x {
            core::mem::swap(&mut r0x, &mut r1x);
        }
        if r0y < r1y {
            core::mem::swap(&mut r0y, &mut r1y);
        }
        if r1x < 0 || r1y < 0 {
            return;
        }
        let ring = (angle0 - angle1).abs() >= 360.0;
        let mut start = math::fmod(angle0, 360.0);
        let mut end = math::fmod(angle1, 360.0);
        if start < 0.0 {
            start = math::fmod(start + 360.0, 360.0);
        }
        if end < 0.0 {
            end = math::fmod(end + 360.0, 360.0);
        }
        if ring && (start - end).abs() <= 0.0001 {
            start = 0.0;
            end = 360.0;
        }
        self.fill_arc_helper(x, y, r0x, r1x, r0y, r1y, start, end, rgb565(color));
    }

    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    fn fill_arc_helper(
        &mut self,
        cx: i32,
        cy: i32,
        oradius_x: i32,
        iradius_x: i32,
        oradius_y: i32,
        iradius_y: i32,
        start: f32,
        end: f32,
        color: u16,
    ) {
        let s_cos = math::cos(start * DEG_TO_RAD);
        let e_cos = math::cos(end * DEG_TO_RAD);
        let sslope = s_cos / math::sin(start * DEG_TO_RAD);
        let eslope = if end == 360.0 { -1_000_000.0 } else { e_cos / math::sin(end * DEG_TO_RAD) };
        let swidth = 0.5 / s_cos;
        let ewidth = -0.5 / e_cos;

        let start180 = start >= 180.0;
        let end180 = end < 180.0;
        let reversed = start + 180.0 < end || (end < start && start < end + 180.0);

        let mut xleft = -oradius_x;
        let mut xright = oradius_x + 1;
        let mut y = -oradius_y;
        let mut ye = oradius_y;
        if !reversed {
            if (end >= 270.0 || end < 90.0) && (start >= 270.0 || start < 90.0) {
                xleft = 0;
            } else if (90.0..270.0).contains(&end) && (90.0..270.0).contains(&start) {
                xright = 1;
            }
            if end >= 180.0 && start >= 180.0 {
                ye = 0;
            } else if end < 180.0 && start < 180.0 {
                y = 0;
            }
        }
        y = y.max(self.clip_t - cy);
        ye = ye.min(self.clip_b - cy + 1);
        xleft = xleft.max(self.clip_l - cx);
        xright = xright.min(self.clip_r - cx + 1);

        let true_circle = oradius_x == oradius_y && iradius_x == iradius_y;

        let iradius_y2 = iradius_y * (iradius_y - 1);
        let iradius_x2 = iradius_x * (iradius_x - 1);
        let irad_rate = if iradius_x2 != 0 && iradius_y2 != 0 {
            iradius_x2 as f32 / iradius_y2 as f32
        } else {
            0.0
        };
        let oradius_y2 = oradius_y * (oradius_y + 1);
        let oradius_x2 = oradius_x * (oradius_x + 1);
        let orad_rate = if oradius_x2 != 0 && oradius_y2 != 0 {
            oradius_x2 as f32 / oradius_y2 as f32
        } else {
            0.0
        };

        loop {
            let y2 = y * y;
            let mut compare_o = oradius_y2 - y2;
            let mut compare_i = iradius_y2 - y2;
            if !true_circle {
                compare_i = math::floor(compare_i as f32 * irad_rate) as i32;
                compare_o = math::ceil(compare_o as f32 * orad_rate) as i32;
            }
            let mut xe = math::ceil(math::sqrt(compare_o as f32)) as i32;
            let mut x = 1 - xe;
            x = x.max(xleft);
            xe = xe.min(xright);
            let ysslope = (y as f32 + swidth) * sslope;
            let yeslope = (y as f32 + ewidth) * eslope;
            let mut len = 0;
            loop {
                let flg1 = start180 != (x as f32 <= ysslope);
                let flg2 = end180 != (x as f32 <= yeslope);
                let x2 = x * x;
                if x2 >= compare_i
                    && ((flg1 && flg2) || (reversed && (flg1 || flg2)))
                    && x != xe
                    && x2 < compare_o
                {
                    len += 1;
                } else {
                    if len != 0 {
                        self.write_fast_hline(cx + x - len, cy + y, len, color);
                        len = 0;
                    }
                    if x2 >= compare_o {
                        break;
                    }
                    if x < 0 && x2 < compare_i {
                        x = -x;
                    }
                }
                x += 1;
                if x > xe {
                    break;
                }
            }
            y += 1;
            if y > ye {
                break;
            }
        }
    }

    // ---------------------------------------------------------------------------------------
    // Text

    /// Code points of `text` as LovyanGFX reads them: decoded on the canvas's running UTF-8
    /// state, control characters skipped, stopping at a NUL or when `f` returns false.
    fn for_each_char(&mut self, text: &[u8], mut f: impl FnMut(&mut Self, u32) -> bool) {
        for &byte in text {
            if byte == 0 {
                break;
            }
            let code = self.utf8.feed(byte);
            if code < 0x20 {
                continue;
            }
            if (0xFE00..0xFE10).contains(&code) {
                continue; // variation selectors
            }
            if !f(self, code) {
                break;
            }
        }
    }

    /// Width in pixels `text` takes up in `font` at `size`.
    pub fn text_width(&mut self, font: &Font<'_>, text: &[u8], size: f32) -> i32 {
        let sx = TextStyle::new(Datum::TopLeft, 0).sized(size).scale();
        let mut left = 0;
        let mut right = 0;
        self.for_each_char(text, |_, code| {
            let (metric, known) = font.metric(code as u16);
            let metric = if code > 0xFFFF || !known { font.metric(0).0 } else { metric };
            let sxoffset = (metric.x_offset * sx) >> 16;
            if left == 0 && right == 0 && metric.x_offset < 0 {
                left = -sxoffset;
                right = left;
            }
            let sxadvance = (metric.x_advance * sx) >> 16;
            right = left + sxadvance.max(((metric.width * sx) >> 16) + sxoffset);
            left += sxadvance;
            true
        });
        right
    }

    /// Draws `text` anchored at `(x, y)`. Returns the advance.
    pub fn draw_string(
        &mut self,
        font: &Font<'_>,
        text: &[u8],
        x: i32,
        y: i32,
        style: TextStyle,
    ) -> i32 {
        let sx = style.scale();
        let sy = sx;
        let cwidth = self.text_width(font, text, style.size);
        let cheight = (font.height() * sy) >> 16;

        // A first glyph that hangs left of the cursor shifts the line right.
        let mut sum_x = 0;
        self.for_each_char(text, |_, code| {
            let (metric, known) = font.metric(code as u16);
            let metric = if code > 0xFFFF || !known { font.metric(0).0 } else { metric };
            if metric.x_offset < 0 {
                sum_x = (-(metric.x_offset * sx)) >> 16;
            }
            false // only the first character counts
        });

        let mut y = y;
        let mut x = x;
        if style.datum.middle() {
            y -= cheight >> 1;
        }
        match style.datum {
            Datum::MiddleCenter => x -= cwidth >> 1,
            Datum::MiddleRight => x -= cwidth,
            Datum::TopLeft | Datum::MiddleLeft => {}
        }

        self.for_each_char(text, |canvas, code| {
            let known = code <= 0xFFFF && font.metric(code as u16).1;
            let code = if known { code as u16 } else { 0 };
            sum_x += canvas.draw_char(font, x + sum_x, y, code, &style);
            true
        });
        sum_x
    }

    fn draw_char(&mut self, font: &Font<'_>, x: i32, y: i32, code: u16, style: &TextStyle) -> i32 {
        let sx = style.scale();
        let sy = sx;
        if code == 0x20 {
            // A space only advances the cursor.
            return (font.space_advance() * sx) >> 16;
        }
        let Some(index) = font.index_of(code) else {
            // Unknown character: an outlined box.
            let w = (font.space_width() * sx) >> 16;
            let h = (font.height() * sy) >> 16;
            if 2 < w && 2 < h {
                self.draw_rect(x + 1, y + 1, w - 2, h - 2, style.color);
            }
            return w;
        };
        let glyph = font.glyph(index);
        let (h, w) = (glyph.height, glyph.width);
        let x_advance = (glyph.x_advance * sx) >> 16;
        let xoffset = (glyph.x_offset * sx) >> 16;
        let yoffset = font.max_ascent() - glyph.d_y;
        let x = x + xoffset;

        let clip_right = self.clip_r;
        let clip_bottom = self.clip_b;
        let mut bx = x;
        let mut bw = (w * sx) >> 16;
        if x < self.clip_l {
            bw += x - self.clip_l;
            bx = self.clip_l;
        }
        bw = bw.min(clip_right + 1 - bx);
        if bw < 0 || w <= 0 || h <= 0 {
            return x_advance;
        }

        let fore_r = (style.color >> 16) & 0xFF;
        let fore_g = (style.color >> 8) & 0xFF;
        let fore_b = style.color & 0xFF;

        let mut y1 = (yoffset * sy) >> 16;
        for i in 0..h {
            let y0 = y1;
            if y0 > clip_bottom - y {
                break;
            }
            y1 = ((yoffset + i + 1) * sy) >> 16;
            let mut by = y + y0;
            let mut bh = y1 - y0;
            if by < self.clip_t {
                bh += by - self.clip_t;
                by = self.clip_t;
            }
            let row = &glyph.pixels[(i * w) as usize..((i + 1) * w) as usize];
            if bh <= 0 {
                continue;
            }
            // First and last covered pixel of the row.
            let Some(j0) = row.iter().position(|&p| p != 0) else { continue };
            let j1 = row.iter().rposition(|&p| p != 0).map_or(j0, |j| j + 1) as i32;
            let j0 = j0 as i32;
            let mut rx = (j0 * sx) >> 16;
            let mut rw = (j1 * sx) >> 16;
            rx = rx.max(bx - x);
            rw = rw.min(bx + bw - x);
            rw -= rx;
            if rw <= 0 {
                continue;
            }
            let mut x1 = ((j0 * sx) >> 16) - rx;
            for j in j0..j1 {
                let mut x0 = x1.max(0);
                x1 = (((j + 1) * sx) >> 16) - rx;
                x1 = x1.min(rw);
                let coverage = row[j as usize];
                if coverage == 0 || x0 >= x1 {
                    continue;
                }
                let p = 1 + u32::from(coverage);
                while x0 < x1 {
                    for yy in 0..bh {
                        let (px, py) = (x + rx + x0, by + yy);
                        if !self.in_clip(px, py) {
                            continue;
                        }
                        let (r, g, b) = self.get_rgb(px, py);
                        let blended = rgb888(
                            ((fore_r * p + u32::from(r) * (257 - p)) >> 8) as u8,
                            ((fore_g * p + u32::from(g) * (257 - p)) >> 8) as u8,
                            ((fore_b * p + u32::from(b) * (257 - p)) >> 8) as u8,
                        );
                        self.store(px, py, rgb565(blended));
                    }
                    x0 += 1;
                }
            }
        }
        x_advance
    }
}

/// Distance from a pixel to the closest part of a tapered segment (TFT_eSPI's formula).
fn wedge_line_distance(xpax: f32, ypay: f32, bax: f32, bay: f32, dr: f32) -> f32 {
    let d = (xpax * bax + ypay * bay) / (bax * bax + bay * bay);
    let h = d.clamp(0.0, 1.0);
    let dx = xpax - bax * h;
    let dy = ypay - bay * h;
    math::sqrt(dx * dx + dy * dy) + h * dr
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::vec;
    use std::vec::Vec;

    use super::*;

    fn canvas() -> Canvas<Vec<u16>> {
        Canvas::new(vec![0; 320 * 170], 320, 170)
    }

    #[test]
    fn colour_conversions_match_lovyangfx() {
        assert_eq!(rgb565(0xFFFFFF), 0xFFFF);
        assert_eq!(expand565(0xFFFF), (255, 255, 255));
        assert_eq!(expand565(rgb565(0x0B0B0D)), (8, 8, 8));
    }

    #[test]
    fn rotation_three_maps_landscape_onto_portrait_storage() {
        let mut c = Canvas::new(vec![0; 170 * 320], 170, 320);
        c.set_rotation(3);
        assert_eq!((c.width(), c.height()), (320, 170));
        c.draw_pixel(0, 0, 0xFFFFFF);
        // Rotation 3: x' = y, y' = width − 1 − x.
        assert_eq!(c.pixels()[319 * 170], 0xFFFF);
    }

    #[test]
    fn clip_rect_limits_fills() {
        let mut c = canvas();
        c.set_clip_rect(10, 10, 5, 5);
        c.fill_screen(0xFFFFFF);
        assert_eq!(c.get(9, 10), 0);
        assert_eq!(c.get(10, 10), 0xFFFF);
        assert_eq!(c.get(14, 14), 0xFFFF);
        assert_eq!(c.get(15, 14), 0);
    }

    #[test]
    fn wedge_clears_the_clip_rect() {
        let mut c = canvas();
        c.set_clip_rect(0, 20, 320, 150);
        c.dot(50.0, 50.0, 3.0, 0xFFFFFF);
        c.fill_rect(0, 0, 1, 1, 0xFFFFFF);
        assert_eq!(c.get(0, 0), 0xFFFF);
    }

    #[test]
    fn rotated_fills_cover_exactly_their_rectangle() {
        for rotation in 0..4 {
            let mut fast = Canvas::new(vec![0; 170 * 320], 170, 320);
            let mut slow = Canvas::new(vec![0; 170 * 320], 170, 320);
            fast.set_rotation(rotation);
            slow.set_rotation(rotation);
            fast.fill_rect(13, 7, 41, 29, 0xFF8040);
            for y in 7..7 + 29 {
                for x in 13..13 + 41 {
                    slow.store(x, y, rgb565(0xFF8040));
                }
            }
            assert_eq!(fast.pixels(), slow.pixels(), "rotation {rotation}");
        }
    }

    #[test]
    fn swapped_storage_reads_back_the_same() {
        let mut c = Canvas::new(vec![0; 320 * 170], 320, 170).byte_swapped();
        c.fill_rect(0, 0, 1, 1, 0xFF0000);
        assert_eq!(c.get(0, 0), 0xF800);
        assert_eq!(c.pixels()[0], 0x00F8);
    }
}
