//! Serial transfer capture for Blargg / Mooneye `$FF01` (SB) oracles.
//!
//! Owns the byte stream written to SB; transfer timing itself stays in the bus I/O path.

/// Bytes captured from `$FF01` writes.
#[derive(Default, Clone)]
pub(super) struct SerialCapture {
    bytes: Vec<u8>,
}

impl SerialCapture {
    pub(super) fn new() -> Self {
        Self::default()
    }

    pub(super) fn push(&mut self, value: u8) {
        self.bytes.push(value);
    }

    pub(super) fn clear(&mut self) {
        self.bytes.clear();
    }

    pub(super) fn as_slice(&self) -> &[u8] {
        &self.bytes
    }

    pub(super) fn text(&self) -> String {
        String::from_utf8_lossy(&self.bytes).into_owned()
    }

    pub(super) fn from_vec(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }
}
