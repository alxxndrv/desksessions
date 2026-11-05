//! Motion: timing curves, Pebble's moook, springs, and the retargetable channel built on them.

mod breathing;
mod channel;
mod easing;
mod marquee;
mod moook;
mod rubber_band;
mod spring;

pub use breathing::{BREATH_PERIOD, BREATH_REST, breathe};
pub use channel::{Channel, MOOOK_SETTLE, Motion};
pub use easing::{
    CA_DEFAULT, CA_EASE_IN, CA_EASE_IN_OUT, CA_EASE_OUT, CubicBezier, Curve, ease, lerp,
};
pub use marquee::{
    MARQUEE_LEAD_PAUSE, MARQUEE_PER_PIXEL, MARQUEE_REWIND_PER_PIXEL, MARQUEE_TAIL_PAUSE,
    marquee_offset,
};
pub use moook::{
    MOOOK, MOOOK_IN, MOOOK_OUT, MOOOK_REFERENCE_DISTANCE, MOOOK_SOFT, MoookShape, PEBBLE_FRAME,
    PEEK, PEEK_OUT, moook,
};
pub use rubber_band::{RUBBER_BAND_C, rubber_band};
pub use spring::{BOUNCY, SMOOTH, SNAPPY, SPRING_STEP, Spring, SpringParams, spring_from};
