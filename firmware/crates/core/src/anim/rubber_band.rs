/// `UIScrollView`'s resistance constant.
pub const RUBBER_BAND_C: f32 = 0.55;

/// `UIScrollView`-style resistance: how far content moves when dragged `offset` past its edge.
/// Behaves like `offset · c` for small offsets and never exceeds `dimension`.
#[must_use]
pub fn rubber_band(offset: f32, dimension: f32, c: f32) -> f32 {
    if dimension <= 0.0 {
        return 0.0;
    }
    let magnitude = (1.0 - 1.0 / (offset.abs() * c / dimension + 1.0)) * dimension;
    magnitude.copysign(offset)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assert_float_eq;

    #[test]
    fn resists_and_never_exceeds_dimension() {
        let band = |offset| rubber_band(offset, 100.0, RUBBER_BAND_C);
        assert_float_eq!(band(0.0), 0.0);
        assert!((band(1.0) - 0.55).abs() < 0.01);
        assert!(band(10_000.0) < 100.0);
        assert_float_eq!(band(-20.0), -band(20.0));
    }
}
