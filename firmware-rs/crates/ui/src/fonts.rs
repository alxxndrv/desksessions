//! Golos Text, anti-aliased, with Cyrillic. Rendered by `tools/make_vlw.py` from
//! `fonts/GolosText[wght].ttf` (SIL Open Font License, see `fonts/OFL.txt`).

use crate::vlw::Font;

pub const GOLOS_16_MEDIUM: &[u8] = include_bytes!("../fonts/golos-16-medium.vlw");
pub const GOLOS_12_REGULAR: &[u8] = include_bytes!("../fonts/golos-12-regular.vlw");
pub const GOLOS_12_MEDIUM: &[u8] = include_bytes!("../fonts/golos-12-medium.vlw");
pub const GOLOS_34_SEMIBOLD: &[u8] = include_bytes!("../fonts/golos-34-semibold.vlw");

/// The four faces the renderer uses, parsed once.
#[derive(Clone, Debug)]
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
    /// # Panics
    ///
    /// Never in practice: the embedded files are checked by the tests.
    #[must_use]
    pub fn load() -> Self {
        let parse = |data| Font::parse(data).expect("embedded font");
        Self {
            name: parse(GOLOS_16_MEDIUM),
            small: parse(GOLOS_12_REGULAR),
            label: parse(GOLOS_12_MEDIUM),
            digits: parse(GOLOS_34_SEMIBOLD),
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
    fn embedded_fonts_parse_with_lovyangfx_metrics() {
        let fonts = Fonts::load();
        assert!(fonts.digits.index_of(u16::from(b'5')).is_some());
        assert!(fonts.name.index_of(0x416).is_some()); // Ж
        assert!(fonts.small.index_of(0x2191).is_some()); // ↑
        assert!(fonts.name.height() > 16);
    }
}
