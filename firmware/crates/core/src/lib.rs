//! The clock's portable core: domain, application logic and animation. No display, no Bluetooth,
//! no allocation, so the same code runs on the ESP32-S3, in the simulator and in tests.

#![no_std]

#[cfg(feature = "std")]
extern crate std;

pub mod anim;
pub mod app;
pub mod domain;
pub mod math;
pub mod protocol;
pub mod scene;
pub mod text;
pub mod time;

pub use time::{Instant, Millis, progress};

/// `EXPECT_FLOAT_EQ`: equal within four units in the last place.
#[doc(hidden)]
#[must_use]
pub fn float_eq(a: f32, b: f32) -> bool {
    if a == b {
        return true;
    }
    if a.is_sign_negative() != b.is_sign_negative() {
        return false;
    }
    a.to_bits().abs_diff(b.to_bits()) <= 4
}

/// Asserts two `f32`s are equal within four units in the last place, like gtest's
/// `EXPECT_FLOAT_EQ`.
#[macro_export]
macro_rules! assert_float_eq {
    ($a:expr, $b:expr $(,)?) => {{
        let (a, b): (f32, f32) = ($a, $b);
        assert!($crate::float_eq(a, b), "{a} != {b}");
    }};
    ($a:expr, $b:expr, $($msg:tt)+) => {{
        let (a, b): (f32, f32) = ($a, $b);
        assert!($crate::float_eq(a, b), "{a} != {b}: {}", format_args!($($msg)+));
    }};
}
