//! OAM DMA (`$FF46`) bus-side helpers: source reads, conflicts, and transfer advance.
//!
//! Transfer counters and OAM locking live in [`crate::ppu::Ppu`]; this module owns the
//! bus conflict rules and the copy from the DMA source bus into OAM.

use super::Bus;

impl Bus {
    /// Copy the next DMA bytes from the source bus into OAM.
    ///
    /// While transferring, CPU OAM access returns `$FF` / ignores writes. Reads
    /// from the DMA source bus return the byte currently in flight (bus conflict).
    pub(super) fn advance_oam_dma(&mut self, t_cycles: u32) {
        let srcs = self.ppu.tick_dma(t_cycles);
        for src in srcs {
            let index = (src & 0xFF) as u8;
            let value = self.read8_dma_source(src);
            self.ppu.dma_write_oam_byte(index, value);
            self.ppu.set_dma_data_byte(value);
        }
    }

    /// DMA source read — may touch ROM/VRAM/WRAM (not OAM recursion).
    ///
    /// `$E000`–`$FFFF` mirrors WRAM (`$C000` | ((addr - `$E000`) & `$1FFF`)).
    pub(super) fn read8_dma_source(&self, addr: u16) -> u8 {
        let cgb = self.cgb_memory();
        match addr {
            0x0000..=0x7FFF | 0xA000..=0xBFFF => self.cartridge.read8(addr),
            0x8000..=0x9FFF => self.ppu.vram.cpu_read(addr, cgb),
            0xC000..=0xDFFF => self.wram.read(addr, cgb),
            0xE000..=0xFFFF => self.wram.read(Self::echo_wram_addr(addr), cgb),
        }
    }

    /// True when a CPU access to `addr` conflicts with the active OAM DMA source bus.
    pub(super) fn dma_conflicts_with(&self, addr: u16) -> bool {
        if !self.ppu.dma_blocks_oam() {
            return false;
        }
        let page = self.ppu.dma_source_page();
        match addr {
            // HRAM / IE never conflict.
            0xFF80..=0xFFFF => false,
            // OAM is locked separately (reads as `$FF`).
            0xFE00..=0xFE9F => false,
            0x0000..=0x7FFF | 0xA000..=0xBFFF => page < 0x80 || (0xA0..0xC0).contains(&page),
            0x8000..=0x9FFF => (0x80..0xA0).contains(&page),
            0xC000..=0xFDFF => page >= 0xC0,
            _ => false,
        }
    }
}
