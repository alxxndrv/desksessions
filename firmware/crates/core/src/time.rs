//! Monotonic time in whole milliseconds.

use core::ops::{Add, AddAssign, Mul, Neg, Sub, SubAssign};

/// A signed span of milliseconds. Spans can be negative: "how long until" is often asked of
/// instants that have not arrived yet.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Millis(pub i64);

impl Millis {
    pub const ZERO: Self = Self(0);

    #[must_use]
    pub const fn new(ms: i64) -> Self {
        Self(ms)
    }

    #[must_use]
    pub const fn count(self) -> i64 {
        self.0
    }

    /// `self` as seconds, the way `std::chrono::duration<float>` converts.
    #[must_use]
    pub fn as_secs_f32(self) -> f32 {
        self.0 as f32 / 1000.0
    }

    /// Whole times `other` fits into `self`, truncating toward zero.
    #[must_use]
    pub const fn div(self, other: Self) -> i64 {
        self.0 / other.0
    }

    /// Remainder of `self / other`, with the sign of `self`.
    #[must_use]
    pub const fn rem(self, other: Self) -> Self {
        Self(self.0 % other.0)
    }
}

impl Add for Millis {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }
}

impl Sub for Millis {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self(self.0 - rhs.0)
    }
}

impl AddAssign for Millis {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

impl SubAssign for Millis {
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0;
    }
}

impl Mul<i64> for Millis {
    type Output = Self;
    fn mul(self, rhs: i64) -> Self {
        Self(self.0 * rhs)
    }
}

impl Neg for Millis {
    type Output = Self;
    fn neg(self) -> Self {
        Self(-self.0)
    }
}

/// A point on the monotonic clock, measured from boot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Instant {
    pub since_boot: Millis,
}

impl Instant {
    #[must_use]
    pub const fn from_millis(ms: i64) -> Self {
        Self { since_boot: Millis(ms) }
    }
}

impl Sub for Instant {
    type Output = Millis;
    fn sub(self, rhs: Self) -> Millis {
        self.since_boot - rhs.since_boot
    }
}

impl Add<Millis> for Instant {
    type Output = Self;
    fn add(self, rhs: Millis) -> Self {
        Self { since_boot: self.since_boot + rhs }
    }
}

impl Sub<Millis> for Instant {
    type Output = Self;
    fn sub(self, rhs: Millis) -> Self {
        Self { since_boot: self.since_boot - rhs }
    }
}

impl AddAssign<Millis> for Instant {
    fn add_assign(&mut self, rhs: Millis) {
        self.since_boot += rhs;
    }
}

/// Fraction of `total` covered by `elapsed`, clamped to [0, 1]. Zero-length spans count as done.
#[must_use]
pub fn progress(elapsed: Millis, total: Millis) -> f32 {
    if total.0 <= 0 || elapsed >= total {
        return 1.0;
    }
    if elapsed.0 <= 0 {
        return 0.0;
    }
    elapsed.0 as f32 / total.0 as f32
}
