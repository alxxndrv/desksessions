//! Golos Text, anti-aliased, with Cyrillic. Rendered by `tools/make_vlw.py` from
//! `fonts/GolosText[wght].ttf` (SIL Open Font License, see `fonts/OFL.txt`).

use crate::vlw::{Font, Table, glyph_count};

const GOLOS_16_MEDIUM: &[u8] = include_bytes!("../fonts/golos-16-medium.vlw");
const GOLOS_12_REGULAR: &[u8] = include_bytes!("../fonts/golos-12-regular.vlw");
const GOLOS_12_MEDIUM: &[u8] = include_bytes!("../fonts/golos-12-medium.vlw");
const GOLOS_34_SEMIBOLD: &[u8] = include_bytes!("../fonts/golos-34-semibold.vlw");

// Glyph tables, unpacked while compiling: no RAM, no parsing at boot.
static NAME: Table<{ glyph_count(GOLOS_16_MEDIUM) }> = Table::new(GOLOS_16_MEDIUM);
static SMALL: Table<{ glyph_count(GOLOS_12_REGULAR) }> = Table::new(GOLOS_12_REGULAR);
static LABEL: Table<{ glyph_count(GOLOS_12_MEDIUM) }> = Table::new(GOLOS_12_MEDIUM);
static DIGITS: Table<{ glyph_count(GOLOS_34_SEMIBOLD) }> = Table::new(GOLOS_34_SEMIBOLD);

/// The four faces the renderer uses.
#[derive(Clone, Copy, Debug)]
pub struct Fonts {
    /// 16 px medium: session names.
    pub name: Font<'static>,
    /// 12 px regular: details, times.
    pub small: Font<'static>,
    /// 12 px medium: header counts.
    pub label: Font<'static>,
    /// 34 px semibold: passkey.
    pub digits: Font<'static>,
}

impl Fonts {
    #[must_use]
    pub fn load() -> Self {
        Self {
            name: NAME.font(GOLOS_16_MEDIUM),
            small: SMALL.font(GOLOS_12_REGULAR),
            label: LABEL.font(GOLOS_12_MEDIUM),
            digits: DIGITS.font(GOLOS_34_SEMIBOLD),
        }
    }
}

impl Default for Fonts {
    fn default() -> Self {
        Self::load()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_fonts_unpack_with_lovyangfx_metrics() {
        let fonts = Fonts::load();
        assert!(fonts.digits.index_of(u16::from(b'5')).is_some());
        assert!(fonts.name.index_of(0x416).is_some()); // Ж
        assert!(fonts.small.index_of(0x2191).is_some()); // ↑
        assert!(fonts.name.index_of(0x4E00).is_none());
        assert!(fonts.name.height() > 16);
    }
}
