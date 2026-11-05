//! Anti-aliased bitmap fonts in the VLW format (Processing's, as LovyanGFX reads it).
//!
//! All integers are 32-bit big-endian:
//!   header: glyph count, version, size, 0, ascent, descent
//!   per glyph, sorted by code point: unicode, height, width, xAdvance, dY (top above baseline),
//!                                    dX (left offset), 0
//!   then each glyph's width × height alpha bitmap, in the same order.
//!
//! Metrics are derived exactly as LovyanGFX derives them, so text sits where it always did.

/// Most glyphs a font may hold.
pub const MAX_GLYPHS: usize = 512;

const HEADER_BYTES: usize = 24;
const GLYPH_HEADER_BYTES: usize = 28;

/// Why a VLW blob was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontError {
    Empty,
    TooManyGlyphs,
    Truncated,
}

/// What text layout needs to know about one glyph.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Metric {
    pub width: i32,
    pub x_advance: i32,
    pub x_offset: i32,
}

/// One glyph, ready to draw.
#[derive(Clone, Copy, Debug)]
pub struct Glyph<'a> {
    pub height: i32,
    pub width: i32,
    pub x_advance: i32,
    /// Top of the bitmap above the baseline.
    pub d_y: i32,
    pub x_offset: i32,
    /// `width × height` coverage values, row by row.
    pub pixels: &'a [u8],
}

/// A parsed VLW font. Borrows the file; only the glyphs' bitmap offsets are computed up front.
#[derive(Clone, Debug)]
pub struct Font<'a> {
    data: &'a [u8],
    count: usize,
    bitmaps: heapless::Vec<u32, MAX_GLYPHS>,
    max_ascent: i32,
    y_advance: i32,
    space_width: i32,
}

fn be32(data: &[u8], at: usize) -> u32 {
    u32::from_be_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]])
}

impl<'a> Font<'a> {
    /// Parses a VLW file.
    ///
    /// # Errors
    ///
    /// Refuses files with no glyphs, more than [`MAX_GLYPHS`], or bitmaps past the end.
    pub fn parse(data: &'a [u8]) -> Result<Self, FontError> {
        if data.len() < HEADER_BYTES {
            return Err(FontError::Truncated);
        }
        let count = be32(data, 0) as usize;
        let size = be32(data, 8) as i32;
        let ascent = (be32(data, 16) as i32).abs();
        let descent = (be32(data, 20) as i32).abs();
        if count == 0 {
            return Err(FontError::Empty);
        }
        if count > MAX_GLYPHS {
            return Err(FontError::TooManyGlyphs);
        }
        if data.len() < HEADER_BYTES + count * GLYPH_HEADER_BYTES {
            return Err(FontError::Truncated);
        }

        let mut max_ascent = ascent;
        let mut max_descent = descent;
        // LovyanGFX guesses the space width from the size before measuring anything.
        let space_width = size.max(ascent + descent) * 2 / 7;

        let mut bitmaps = heapless::Vec::new();
        let mut bitmap = (HEADER_BYTES + count * GLYPH_HEADER_BYTES) as u32;
        for i in 0..count {
            let at = HEADER_BYTES + i * GLYPH_HEADER_BYTES;
            let unicode = be32(data, at) as u16;
            let height = be32(data, at + 4) as u16;
            let width = be32(data, at + 8) as u8;
            if unicode > 0xFF || (unicode > 0x20 && unicode < 0xA0 && unicode != 0x7F) {
                let d_y = i32::from(be32(data, at + 16) as i16);
                if max_ascent < d_y && unicode != 0x3000 {
                    max_ascent = d_y;
                }
                if max_descent < i32::from(height) - d_y && unicode != 0x3000 {
                    max_descent = i32::from(height) - d_y;
                }
            }
            // Cannot fail: count ≤ MAX_GLYPHS.
            let _ = bitmaps.push(bitmap);
            bitmap += u32::from(width) * u32::from(height);
        }
        if bitmap as usize > data.len() {
            return Err(FontError::Truncated);
        }

        Ok(Self {
            data,
            count,
            bitmaps,
            max_ascent,
            y_advance: max_ascent + max_descent,
            space_width,
        })
    }

    /// Line height: tallest ascent plus deepest descent.
    #[must_use]
    pub fn height(&self) -> i32 {
        self.y_advance
    }

    #[must_use]
    pub fn max_ascent(&self) -> i32 {
        self.max_ascent
    }

    #[must_use]
    pub fn space_width(&self) -> i32 {
        self.space_width
    }

    fn header(&self, index: usize, field: usize) -> u32 {
        be32(self.data, HEADER_BYTES + index * GLYPH_HEADER_BYTES + field * 4)
    }

    fn unicode_at(&self, index: usize) -> u16 {
        self.header(index, 0) as u16
    }

    /// Index of the glyph for `code`, if the font has one.
    #[must_use]
    pub fn index_of(&self, code: u16) -> Option<usize> {
        if self.unicode_at(self.count - 1) < code {
            return None;
        }
        // lower_bound over the sorted code points
        let (mut low, mut high) = (0, self.count);
        while low < high {
            let mid = (low + high) / 2;
            if self.unicode_at(mid) < code {
                low = mid + 1;
            } else {
                high = mid;
            }
        }
        (low < self.count && self.unicode_at(low) == code).then_some(low)
    }

    /// Layout metrics for `code`. Unknown characters measure as a space; returns whether the
    /// font knows the character (a space always counts as known).
    #[must_use]
    pub fn metric(&self, code: u16) -> (Metric, bool) {
        match self.index_of(code) {
            Some(i) => (
                Metric {
                    width: i32::from(self.header(i, 2) as u8),
                    x_advance: i32::from(self.header(i, 3) as u8),
                    x_offset: i32::from(self.header(i, 5) as i8),
                },
                true,
            ),
            None => (
                Metric { width: self.space_width, x_advance: self.space_width, x_offset: 0 },
                code == 0x20,
            ),
        }
    }

    /// Advance of a space: the font's own space glyph's, else the guess.
    #[must_use]
    pub fn space_advance(&self) -> i32 {
        match self.index_of(0x20) {
            Some(i) => self.header(i, 3) as i32,
            None => self.space_width,
        }
    }

    #[must_use]
    pub fn glyph(&self, index: usize) -> Glyph<'a> {
        let height = self.header(index, 1) as i32;
        let width = self.header(index, 2) as i32;
        let start = self.bitmaps[index] as usize;
        let len = (width.max(0) * height.max(0)) as usize;
        Glyph {
            height,
            width,
            x_advance: self.header(index, 3) as i32,
            d_y: i32::from(self.header(index, 4) as i16),
            x_offset: i32::from(self.header(index, 5) as i8),
            pixels: &self.data[start..start + len],
        }
    }
}
