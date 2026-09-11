//! CGB LCD VRAM DMA (`$FF51–$FF55`) orchestration on the bus.
//!
//! Transfer state lives in [`crate::hw::VramDma`]; this module runs GDMA/HDMA block
//! copies and the nested [`Bus::tick`] stalls that accompany each block.

use super::Bus;
use crate::hw::{BLOCK_LEN, Block, StartOutcome, VramDmaMode, block_cpu_t, hblank_may_transfer};
use crate::ppu::{PpuMode, VBLANK_LINE};

impl Bus {
    pub(super) fn currently_hblank(&self) -> bool {
        self.ppu.lcd_enabled() && self.ppu.mode() == PpuMode::HBlank && self.ppu.ly() < VBLANK_LINE
    }

    pub(super) fn copy_vram_dma_block(&mut self, block: Block) {
        let cgb = self.cgb_memory();
        for i in 0..BLOCK_LEN {
            let value = if block.garbage_src {
                0xFF
            } else {
                self.read8(block.src.wrapping_add(i))
            };
            self.ppu
                .vram
                .cpu_write(block.dest.wrapping_add(i), value, cgb);
        }
    }

    pub(super) fn run_gdma(&mut self) {
        self.gdma_running = true;
        while let Some(block) = self.vram_dma.take_block() {
            self.copy_vram_dma_block(block);
            let stall = block_cpu_t(self.clock_state());
            self.tick(stall);
        }
        self.gdma_running = false;
    }

    pub(super) fn run_hdma_hblank_bursts(&mut self, hblank_lines: &[u8]) {
        if self.vram_dma.mode() != VramDmaMode::Hdma {
            return;
        }
        for &ly in hblank_lines {
            if self.vram_dma.mode() != VramDmaMode::Hdma {
                break;
            }
            if !hblank_may_transfer(ly, self.cpu_halted) {
                continue;
            }
            self.vram_dma.allow_next_hblank_block();
            if let Some(block) = self.vram_dma.take_block() {
                self.copy_vram_dma_block(block);
                self.hdma_burst = true;
                let stall = block_cpu_t(self.clock_state());
                self.tick(stall);
                self.hdma_burst = false;
            }
        }
    }

    pub(super) fn start_vram_dma(&mut self, value: u8) {
        match self
            .vram_dma
            .start_from_hdma5(value, self.currently_hblank())
        {
            StartOutcome::StartedGdma => self.run_gdma(),
            StartOutcome::StartedHdma { .. } | StartOutcome::AbortedHdma { .. } => {}
        }
    }
}
