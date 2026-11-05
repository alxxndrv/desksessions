//! Float functions `core` lacks.
//!
//! Rounding, square roots and remainders are exact in IEEE 754, so `libm` gives the same bits
//! everywhere. Trigonometry is not: with the `std` feature (host builds) it comes from the
//! platform's libm, as it did in the C++ simulator, so pictures match it closely.

#[must_use]
pub fn sin(x: f32) -> f32 {
    #[cfg(feature = "std")]
    {
        std::primitive::f32::sin(x)
    }
    #[cfg(not(feature = "std"))]
    {
        libm::sinf(x)
    }
}

#[must_use]
pub fn cos(x: f32) -> f32 {
    #[cfg(feature = "std")]
    {
        std::primitive::f32::cos(x)
    }
    #[cfg(not(feature = "std"))]
    {
        libm::cosf(x)
    }
}

#[must_use]
pub fn powf(x: f32, y: f32) -> f32 {
    #[cfg(feature = "std")]
    {
        std::primitive::f32::powf(x, y)
    }
    #[cfg(not(feature = "std"))]
    {
        libm::powf(x, y)
    }
}

#[must_use]
pub fn floor(x: f32) -> f32 {
    libm::floorf(x)
}

#[must_use]
pub fn ceil(x: f32) -> f32 {
    libm::ceilf(x)
}

#[must_use]
pub fn sqrt(x: f32) -> f32 {
    libm::sqrtf(x)
}

#[must_use]
pub fn fmod(x: f32, y: f32) -> f32 {
    libm::fmodf(x, y)
}

/// `std::lround`: nearest integer, halves away from zero.
#[must_use]
pub fn lround(x: f32) -> i32 {
    libm::roundf(x) as i32
}
