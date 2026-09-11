//! HDMA/GDMA block timing and HBlank/HALT gates. No MMIO, no copy.
//!
//! [Pan Docs](https://gbdev.io/pandocs/CGB_Registers.html#lcd-vram-dma-transfers):
//! each `$10`-byte burst takes ~8 µs in both speeds — 8 M-cycles (Normal) or
//! 16 fast M-cycles (Double). HDMA is not doubled (PPU cadence unchanged).

use crate::hw::ClockState;

pub const BLOCK_LEN: u16 = 16;

/// Visible scanlines that may run an HDMA burst (Pan Docs LY=0–143).
const LAST_VISIBLE_LY: u8 = 143;

/// CPU T-cycles the CPU is stalled for one $10-byte burst.
/// Normal: 8 M-cycles * 4 = 32. Double: 16 fast M-cycles * 4 = 64.
pub fn block_cpu_t(clock: ClockState) -> u32 {
    match clock {
        ClockState::Normal => 32,
        ClockState::Double => 64,
    }
}

/// PPU/fixed-dot T for one burst (not doubled). Always 32.
pub fn block_fixed_t() -> u32 {
    32
}

/// HDMA may copy this HBlank if LY is 0..=143 and CPU is not halted.
/// VBlank LY 144..=153: false. Does not inspect STAT; caller only invokes on Mode 0 entry.
pub fn hblank_may_transfer(ly: u8, cpu_halted: bool) -> bool {
    !cpu_halted && ly <= LAST_VISIBLE_LY
}

#[cfg(test)]
mod tests;
