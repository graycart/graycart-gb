//! VRAM DMA transfer model (blocks, start/abort). No clock, no memory copy.
//!
//! GDMA may `take_block` immediately. HDMA never copies at `$FF55` write; blocks
//! are released only after `allow_next_hblank_block` (Wave 2 G: Mode 0 **enter**).
//! Starting mid-HBlank still does not copy immediately (`defer_this_hblank`);
//! the next Mode 0 enter is the first allow and must arm a block.

use super::mmio::{mask_dest, mask_source, parse_hdma5_write, read_hdma5, source_is_vram};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VramDmaMode {
    Idle,
    Gdma,
    Hdma,
}

#[derive(Debug)]
pub struct VramDma {
    src: u16,
    dest: u16,
    remaining_blocks: u8,
    mode: VramDmaMode,
    /// HDMA: next `take_block` may copy one `$10` window.
    hdma_ready: bool,
    /// `$FF55` remaining_minus_1 while Idle (`0xFF` complete; abort keeps count).
    idle_remaining_minus_1: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Block {
    pub src: u16,
    pub dest: u16,
    pub garbage_src: bool, // source was VRAM
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartOutcome {
    StartedGdma,
    StartedHdma { defer_this_hblank: bool },
    AbortedHdma { remaining_minus_1: u8 },
}

impl Default for VramDma {
    fn default() -> Self {
        Self::new()
    }
}

impl VramDma {
    pub fn new() -> Self {
        Self {
            src: mask_source(0),
            dest: mask_dest(0),
            remaining_blocks: 0,
            mode: VramDmaMode::Idle,
            hdma_ready: false,
            idle_remaining_minus_1: 0xFF,
        }
    }

    pub fn mode(&self) -> VramDmaMode {
        self.mode
    }

    /// 0 when idle; else count of `$10` blocks left (`1..=0x80`).
    pub fn remaining_blocks(&self) -> u8 {
        match self.mode {
            VramDmaMode::Idle => 0,
            VramDmaMode::Gdma | VramDmaMode::Hdma => self.remaining_blocks,
        }
    }

    pub fn write_src_high(&mut self, v: u8) {
        self.src = mask_source((u16::from(v) << 8) | (self.src & 0x00FF));
    }

    pub fn write_src_low(&mut self, v: u8) {
        self.src = mask_source((self.src & 0xFF00) | u16::from(v));
    }

    pub fn write_dest_high(&mut self, v: u8) {
        self.dest = mask_dest((u16::from(v) << 8) | (self.dest & 0x00FF));
    }

    pub fn write_dest_low(&mut self, v: u8) {
        self.dest = mask_dest((self.dest & 0xFF00) | u16::from(v));
    }

    /// Latch HDMA5. If already Hdma and value bit7=0: abort (do not start GDMA).
    pub fn start_from_hdma5(&mut self, value: u8, currently_hblank: bool) -> StartOutcome {
        if self.mode == VramDmaMode::Hdma && value & 0x80 == 0 {
            let remaining_minus_1 = self.abort_hdma();
            return StartOutcome::AbortedHdma { remaining_minus_1 };
        }

        let parsed = parse_hdma5_write(value);
        self.remaining_blocks = parsed.blocks_minus_1 + 1;
        self.idle_remaining_minus_1 = 0xFF;

        if parsed.hblank {
            self.mode = VramDmaMode::Hdma;
            // Never copy at FF55. `currently_hblank` only sets `defer_this_hblank`
            // for the Bus; Wave 2 G allows on Mode 0 enter, so do not skip it.
            self.hdma_ready = false;
            StartOutcome::StartedHdma {
                defer_this_hblank: currently_hblank,
            }
        } else {
            self.mode = VramDmaMode::Gdma;
            self.hdma_ready = false;
            StartOutcome::StartedGdma
        }
    }

    /// Remaining_minus_1 after abort; mode Idle. Latches unchanged.
    pub fn abort_hdma(&mut self) -> u8 {
        let remaining_minus_1 = if self.mode == VramDmaMode::Idle || self.remaining_blocks == 0 {
            0xFF
        } else {
            self.remaining_blocks - 1
        };
        self.mode = VramDmaMode::Idle;
        self.remaining_blocks = 0;
        self.hdma_ready = false;
        self.idle_remaining_minus_1 = remaining_minus_1;
        remaining_minus_1
    }

    /// One `$10` window. None if idle, or HDMA waiting/deferred.
    pub fn take_block(&mut self) -> Option<Block> {
        match self.mode {
            VramDmaMode::Idle => None,
            VramDmaMode::Gdma => self.take_one_block(),
            VramDmaMode::Hdma => {
                if !self.hdma_ready {
                    return None;
                }
                self.hdma_ready = false;
                self.take_one_block()
            }
        }
    }

    /// Arm one `$10` block for the HBlank that is **entering** (Mode 0 enter).
    pub fn allow_next_hblank_block(&mut self) {
        if self.mode != VramDmaMode::Hdma {
            return;
        }
        self.hdma_ready = true;
    }

    pub fn hdma5_read(&self) -> u8 {
        match self.mode {
            VramDmaMode::Idle => read_hdma5(self.idle_remaining_minus_1, false),
            VramDmaMode::Gdma | VramDmaMode::Hdma => {
                read_hdma5(self.remaining_blocks.saturating_sub(1), true)
            }
        }
    }

    fn take_one_block(&mut self) -> Option<Block> {
        if self.remaining_blocks == 0 {
            self.mode = VramDmaMode::Idle;
            return None;
        }
        let block = Block {
            src: self.src,
            dest: self.dest,
            garbage_src: source_is_vram(self.src),
        };
        self.src = mask_source(self.src.wrapping_add(0x10));
        self.dest = mask_dest(self.dest.wrapping_add(0x10));
        self.remaining_blocks -= 1;
        if self.remaining_blocks == 0 {
            self.mode = VramDmaMode::Idle;
            self.hdma_ready = false;
            self.idle_remaining_minus_1 = 0xFF;
        }
        Some(block)
    }
}

#[cfg(test)]
mod tests;
