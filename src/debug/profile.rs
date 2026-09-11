//! Optional Instant-based tick profiling for Bus (observation / Debug Monitor).

use std::time::Duration;

/// Accumulated wall time inside one emulated frame's [`crate::bus::Bus::tick`] calls.
#[derive(Debug, Clone, Copy, Default)]
pub struct TickProfile {
    pub timer: Duration,
    pub dma: Duration,
    pub cart: Duration,
    pub apu: Duration,
    pub ppu: Duration,
}

impl TickProfile {
    pub fn total(self) -> Duration {
        self.timer + self.dma + self.cart + self.apu + self.ppu
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}
