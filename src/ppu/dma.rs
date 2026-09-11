//! OAM DMA (`$FF46`) — copy 160 bytes to OAM over 160 M-cycles.
//!
//! [Pan Docs — OAM DMA](https://gbdev.io/pandocs/OAM_DMA_Transfer.html)
//! [GBEDG — DMA](https://hacktix.github.io/GBEDG/dma/)
//!
//! Timing (Mooneye `oam_dma_start` / `oam_dma_timing`):
//! - Fresh write: M0 = write, M1 = idle (OAM still CPU-accessible), M2 = transfer begins
//! - Restart while active: previous DMA continues through M1, new DMA begins at M2
//! - The write instruction's last M-cycle must not advance the start delay (bus suppress)

/// Bytes transferred per DMA (full OAM).
pub const DMA_BYTE_COUNT: u8 = 160;

/// 1 M-cycle (4 T) after a `$FF46` write before the (new) transfer begins.
const START_DELAY_T: u32 = 4;

#[derive(Debug, Clone)]
pub struct OamDma {
    active: bool,
    /// True while OAM is locked for the CPU (transfer in progress).
    transferring: bool,
    page: u8,
    index: u8,
    cycle_accum: u32,
    /// Countdown before a fresh/restarted transfer begins copying.
    start_delay: u32,
    pending_page: Option<u8>,
    last_value: u8,
    /// Byte most recently written into OAM by DMA (bus-conflict readback).
    data_byte: u8,
}

impl Default for OamDma {
    fn default() -> Self {
        Self::new()
    }
}

impl OamDma {
    pub fn new() -> Self {
        Self {
            active: false,
            transferring: false,
            page: 0,
            index: 0,
            cycle_accum: 0,
            start_delay: 0,
            pending_page: None,
            last_value: 0xFF,
            data_byte: 0xFF,
        }
    }

    pub fn active(&self) -> bool {
        self.active
    }

    pub fn blocks_oam(&self) -> bool {
        self.transferring
    }

    pub fn last_value(&self) -> u8 {
        self.last_value
    }

    pub fn page(&self) -> u8 {
        self.page
    }

    pub fn data_byte(&self) -> u8 {
        self.data_byte
    }

    pub fn set_data_byte(&mut self, value: u8) {
        self.data_byte = value;
    }

    pub fn start(&mut self, value: u8) {
        self.last_value = value;
        if self.transferring && self.index < DMA_BYTE_COUNT {
            // Mid-transfer restart: previous copy continues one M-cycle.
            self.pending_page = Some(value);
            self.start_delay = START_DELAY_T;
            self.active = true;
            return;
        }
        // Fresh start (or replace a pending fresh start).
        self.page = value;
        self.index = 0;
        self.cycle_accum = 0;
        self.start_delay = START_DELAY_T;
        self.pending_page = None;
        self.transferring = false;
        self.active = true;
    }

    pub fn tick(&mut self, mut t_cycles: u32) -> Vec<u16> {
        let mut srcs = Vec::new();
        if !self.active || t_cycles == 0 {
            return srcs;
        }

        while t_cycles > 0 && self.active {
            if self.start_delay > 0 {
                let step = t_cycles.min(self.start_delay);
                if self.transferring && self.index < DMA_BYTE_COUNT {
                    self.advance_copy(step, &mut srcs);
                }
                self.start_delay = self.start_delay.saturating_sub(step);
                t_cycles -= step;
                if self.start_delay == 0 {
                    self.apply_start();
                }
                continue;
            }

            self.advance_copy(t_cycles, &mut srcs);
            t_cycles = 0;
        }

        srcs
    }

    fn apply_start(&mut self) {
        if let Some(page) = self.pending_page.take() {
            self.page = page;
            self.index = 0;
            self.cycle_accum = 0;
        }
        self.transferring = true;
        self.active = true;
    }

    fn advance_copy(&mut self, t_cycles: u32, srcs: &mut Vec<u16>) {
        if !self.transferring || self.index >= DMA_BYTE_COUNT || t_cycles == 0 {
            return;
        }
        self.cycle_accum += t_cycles;
        while self.index < DMA_BYTE_COUNT && self.cycle_accum >= 4 {
            self.cycle_accum -= 4;
            let src = u16::from(self.page) << 8 | u16::from(self.index);
            srcs.push(src);
            self.index = self.index.wrapping_add(1);
            if self.index >= DMA_BYTE_COUNT {
                self.finish();
                return;
            }
        }
    }

    fn finish(&mut self) {
        self.transferring = false;
        self.active = false;
        self.cycle_accum = 0;
        self.start_delay = 0;
        self.pending_page = None;
    }

    pub(crate) fn snapshot(&self) -> (u8, bool, u16, u8, u8) {
        (
            self.last_value,
            self.active,
            u16::from(self.page) << 8,
            self.index,
            self.start_delay.min(255) as u8,
        )
    }

    pub(crate) fn apply_snapshot(&mut self, dma: u8, active: bool, src: u16, index: u8, delay: u8) {
        self.last_value = dma;
        self.active = active;
        self.page = (src >> 8) as u8;
        self.index = index;
        self.start_delay = u32::from(delay);
        self.transferring = active && delay == 0 && index < DMA_BYTE_COUNT;
        self.cycle_accum = 0;
        self.pending_page = None;
        self.data_byte = 0xFF;
    }
}

#[cfg(test)]
mod tests;
