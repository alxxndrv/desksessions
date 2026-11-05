//! Fixed-capacity text as it travels over the wire: bytes, normally UTF-8.
//!
//! The Mac only sends whole UTF-8 sequences, but the clock must not trust that: whatever bytes
//! arrive are kept, shown as best they can be, and sent back cut on sequence boundaries.

use core::fmt;

/// Up to `N` bytes of (normally UTF-8) text. Anything longer is cut at `N` bytes.
#[derive(Clone, Default, PartialEq, Eq, Hash)]
pub struct Text<const N: usize> {
    bytes: heapless::Vec<u8, N>,
}

impl<const N: usize> Text<N> {
    #[must_use]
    pub const fn new() -> Self {
        Self { bytes: heapless::Vec::new() }
    }

    /// The first `N` bytes of `bytes`.
    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut text = Self::new();
        text.push_bytes(bytes);
        text
    }

    /// Appends as much of `bytes` as fits.
    pub fn push_bytes(&mut self, bytes: &[u8]) {
        let room = N - self.bytes.len();
        let take = bytes.len().min(room);
        // Cannot fail: `take` fits.
        let _ = self.bytes.extend_from_slice(&bytes[..take]);
    }

    pub fn push_str(&mut self, text: &str) {
        self.push_bytes(text.as_bytes());
    }

    pub fn clear(&mut self) {
        self.bytes.clear();
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

impl<const N: usize> From<&str> for Text<N> {
    fn from(text: &str) -> Self {
        Self::from_bytes(text.as_bytes())
    }
}

impl<const N: usize> fmt::Debug for Text<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("\"")?;
        for chunk in self.bytes.utf8_chunks() {
            f.write_str(chunk.valid())?;
            if !chunk.invalid().is_empty() {
                f.write_str("\u{FFFD}")?;
            }
        }
        f.write_str("\"")
    }
}

impl<const N: usize> PartialEq<&str> for Text<N> {
    fn eq(&self, other: &&str) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}

/// Length of the longest prefix of `bytes[..n]` that does not end inside a UTF-8 sequence.
#[must_use]
pub fn complete_utf8_prefix(bytes: &[u8], n: usize) -> usize {
    if n == 0 {
        return 0;
    }
    let mut start = n - 1;
    while start > 0 && bytes[start] & 0xC0 == 0x80 {
        start -= 1;
    }
    let lead = bytes[start];
    let need = if lead < 0x80 {
        1
    } else if lead >= 0xF0 {
        4
    } else if lead >= 0xE0 {
        3
    } else {
        2
    };
    if start + need <= n { n } else { start }
}
