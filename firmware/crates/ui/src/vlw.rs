//! Anti-aliased bitmap fonts in the VLW format (Processing's, as LovyanGFX reads it).
//!
//! All integers are 32-bit big-endian:
//!   header: glyph count, version, size, 0, ascent, descent
//!   per glyph, sorted by code point: unicode, height, width, xAdvance, dY (top above baseline),
//!                                    dX (left offset), 0
//!   then each glyph's width × height alpha bitmap, in the same order.
//!
//! Metrics are derived exactly as LovyanGFX derives them, so text sits where it always did.

const HEADER_BYTES: usize = 24;
const GLYPH_HEADER_BYTES: usize = 28;

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

/// One glyph's header, unpacked.
#[derive(Clone, Copy, Debug)]
pub struct Entry {
    unicode: u16,
    height: u16,
    /// As LovyanGFX keeps it: the low byte.
    width: u8,
    x_advance: u8,
    x_offset: i8,
    d_y: i16,
    /// Full header values, for drawing.
    full_width: u32,
    full_x_advance: u32,
    bitmap: u32,
}

impl Entry {
    const EMPTY: Self = Self {
        unicode: 0,
        height: 0,
        width: 0,
        x_advance: 0,
        x_offset: 0,
        d_y: 0,
        full_width: 0,
        full_x_advance: 0,
        bitmap: 0,
    };
}

/// No glyph in [`Table::ascii`].
const NONE: u16 = u16::MAX;

const fn be32(data: &[u8], at: usize) -> u32 {
    u32::from_be_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]])
}

/// Number of glyphs in a VLW file.
#[must_use]
pub const fn glyph_count(data: &[u8]) -> usize {
    be32(data, 0) as usize
}

/// A VLW file's glyph table, unpacked at compile time the way LovyanGFX unpacks it at load time,
/// so text layout never searches the file and the table costs no RAM.
#[derive(Debug)]
pub struct Table<const N: usize> {
    entries: [Entry; N],
    /// Glyph index of each printable ASCII character, for the common case.
    ascii: [u16; 96],
    max_ascent: i32,
    y_advance: i32,
    space_width: i32,
}

impl<const N: usize> Table<N> {
    /// Unpacks `data`, which must hold `N` glyphs.
    ///
    /// # Panics
    ///
    /// On a malformed file; used in constants, that is a build error.
    #[must_use]
    pub const fn new(data: &[u8]) -> Self {
        assert!(data.len() >= HEADER_BYTES, "VLW file truncated");
        assert!(glyph_count(data) == N && N > 0, "VLW glyph count");
        assert!(data.len() >= HEADER_BYTES + N * GLYPH_HEADER_BYTES, "VLW headers truncated");
        let size = be32(data, 8) as i32;
        let ascent = (be32(data, 16) as i32).abs();
        let descent = (be32(data, 20) as i32).abs();

        let mut max_ascent = ascent;
        let mut max_descent = descent;
        // LovyanGFX guesses the space width from the size before measuring anything.
        let space_width = if size > ascent + descent { size } else { ascent + descent } * 2 / 7;

        let mut entries = [Entry::EMPTY; N];
        let mut ascii = [NONE; 96];
        let mut bitmap = (HEADER_BYTES + N * GLYPH_HEADER_BYTES) as u32;
        let mut i = 0;
        while i < N {
            let at = HEADER_BYTES + i * GLYPH_HEADER_BYTES;
            let unicode = be32(data, at) as u16;
            let height = be32(data, at + 4) as u16;
            let width = be32(data, at + 8) as u8;
            let d_y = be32(data, at + 16) as i16;
            if unicode > 0xFF || (unicode > 0x20 && unicode < 0xA0 && unicode != 0x7F) {
                let d_y = d_y as i32;
                if max_ascent < d_y && unicode != 0x3000 {
                    max_ascent = d_y;
                }
                if max_descent < height as i32 - d_y && unicode != 0x3000 {
                    max_descent = height as i32 - d_y;
                }
            }
            if unicode >= 0x20 && unicode < 0x80 && ascii[(unicode - 0x20) as usize] == NONE {
                ascii[(unicode - 0x20) as usize] = i as u16;
            }
            entries[i] = Entry {
                unicode,
                height,
                width,
                x_advance: be32(data, at + 12) as u8,
                x_offset: be32(data, at + 20) as i8,
                d_y,
                full_width: be32(data, at + 8),
                full_x_advance: be32(data, at + 12),
                bitmap,
            };
            bitmap += width as u32 * height as u32;
            i += 1;
        }
        assert!(bitmap as usize <= data.len(), "VLW bitmaps truncated");
        Self { entries, ascii, max_ascent, y_advance: max_ascent + max_descent, space_width }
    }

    /// The font: this table over the file it came from.
    #[must_use]
    pub const fn font<'a>(&'a self, data: &'a [u8]) -> Font<'a> {
        Font {
            data,
            entries: &self.entries,
            ascii: &self.ascii,
            max_ascent: self.max_ascent,
            y_advance: self.y_advance,
            space_width: self.space_width,
        }
    }
}

/// A VLW font: the file's bitmaps and its unpacked [`Table`].
#[derive(Clone, Copy, Debug)]
pub struct Font<'a> {
    data: &'a [u8],
    entries: &'a [Entry],
    ascii: &'a [u16; 96],
    max_ascent: i32,
    y_advance: i32,
    space_width: i32,
}

impl<'a> Font<'a> {
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

    /// Index of the glyph for `code`, if the font has one.
    #[must_use]
    pub fn index_of(&self, code: u16) -> Option<usize> {
        if (0x20..0x80).contains(&code) {
            let i = self.ascii[usize::from(code - 0x20)];
            return (i != NONE).then_some(usize::from(i));
        }
        // lower_bound over the sorted code points
        let i = self.entries.partition_point(|e| e.unicode < code);
        (i < self.entries.len() && self.entries[i].unicode == code).then_some(i)
    }

    /// Layout metrics for `code`. Unknown characters measure as a space; returns whether the
    /// font knows the character (a space always counts as known).
    #[must_use]
    pub fn metric(&self, code: u16) -> (Metric, bool) {
        match self.index_of(code) {
            Some(i) => {
                let e = &self.entries[i];
                (
                    Metric {
                        width: i32::from(e.width),
                        x_advance: i32::from(e.x_advance),
                        x_offset: i32::from(e.x_offset),
                    },
                    true,
                )
            }
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
            Some(i) => self.entries[i].full_x_advance as i32,
            None => self.space_width,
        }
    }

    #[must_use]
    pub fn glyph(&self, index: usize) -> Glyph<'a> {
        let e = &self.entries[index];
        let height = i32::from(e.height);
        let width = e.full_width as i32;
        let start = e.bitmap as usize;
        let len = (width.max(0) * height.max(0)) as usize;
        Glyph {
            height,
            width,
            x_advance: e.full_x_advance as i32,
            d_y: i32::from(e.d_y),
            x_offset: i32::from(e.x_offset),
            pixels: &self.data[start..start + len],
        }
    }
}
