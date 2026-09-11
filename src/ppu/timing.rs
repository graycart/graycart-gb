//! PPU mode / LY timing (Phase 4C / 7C).
//!
//! Variable Mode 3 length is sampled at the Mode 2→3 edge via `mode0_start`.
//!
//! Mode 2 STAT: interrupt source can rise in the final M-cycle of the preceding
//! line (dot 452) before the line counter wraps — coffee-gb / SameBoy. CGB
//! silicon also raises that source on LY=143→144 (one M-cycle before VBlank
//! IF). Full LY-at-452 + 1-T readable-mode lag needs a dot-level PPU scheduler
//! (7C ridge).

use super::registers::{LCDC_ENABLE, Registers, STAT_WRITE_MASK};

/// T-cycles per scanline.
pub const CYCLES_PER_LINE: u32 = 456;
/// Scanlines per frame (0..=153).
pub const LINES_PER_FRAME: u8 = 154;
/// First VBlank scanline.
pub const VBLANK_LINE: u8 = 144;

/// Mode 2 (OAM scan) length.
pub const MODE2_DOTS: u32 = 80;
/// Mode 3 (pixel transfer) base length — see [`super::mode3`] for penalties.
pub const MODE3_DOTS: u32 = 172;
/// Mode 0 starts after Mode 2 + base Mode 3 (no penalties).
pub const MODE0_START: u32 = MODE2_DOTS + MODE3_DOTS; // 252

/// Mode 2 STAT source may rise in the final M-cycle of the preceding line.
pub const MODE2_STAT_EARLY: u32 = CYCLES_PER_LINE - 4; // 452

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PpuMode {
    HBlank = 0,
    VBlank = 1,
    OamScan = 2,
    PixelTransfer = 3,
}

impl PpuMode {
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    pub fn from_u8(v: u8) -> Self {
        match v & 0x03 {
            0 => Self::HBlank,
            1 => Self::VBlank,
            2 => Self::OamScan,
            _ => Self::PixelTransfer,
        }
    }
}

/// Events produced by [`Timing::tick`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TimingEvents {
    pub vblank: bool,
    pub stat: bool,
}

#[derive(Debug, Clone)]
pub struct Timing {
    pub ly: u8,
    pub line_cycles: u32,
    pub mode: PpuMode,
    /// Dot within the line where Mode 0 begins (`MODE2 + mode3_length`).
    pub mode0_start: u32,
    pub(crate) stat_line: bool,
    /// CGB silicon: Mode 2 STAT source also on the last M-cycle of LY=143.
    cgb_silicon: bool,
}

impl Timing {
    pub fn new() -> Self {
        let mut t = Self {
            ly: 0,
            line_cycles: 0,
            mode: PpuMode::OamScan,
            mode0_start: MODE0_START,
            stat_line: false,
            cgb_silicon: false,
        };
        t.mode = t.compute_mode();
        t
    }

    pub fn reset_line_state(&mut self, mode: PpuMode) {
        self.ly = 0;
        self.line_cycles = 0;
        self.mode = mode;
        self.mode0_start = MODE0_START;
    }

    pub fn set_cgb_silicon(&mut self, enabled: bool) {
        self.cgb_silicon = enabled;
    }

    pub fn sync_stat_from_regs(&mut self, regs: &Registers) {
        self.stat_line = self.stat_conditions_met(regs);
    }

    pub fn compute_mode(&self) -> PpuMode {
        if self.ly >= VBLANK_LINE {
            PpuMode::VBlank
        } else if self.line_cycles < MODE2_DOTS {
            PpuMode::OamScan
        } else if self.line_cycles < self.mode0_start {
            PpuMode::PixelTransfer
        } else {
            PpuMode::HBlank
        }
    }

    fn next_ly(&self) -> u8 {
        let next = self.ly.wrapping_add(1);
        if next >= LINES_PER_FRAME { 0 } else { next }
    }

    /// Internal Mode 2 (OAM) STAT source — includes the pre-wrap early window.
    ///
    /// DMG: early window only when the next line is still a draw line (`LY < 144`).
    /// CGB silicon: the same window also fires on the last M-cycle of LY=143
    /// (Gekkio `vblank_stat_intr-C`: Mode 2 STAT one M-cycle / 4 T before VBlank IF).
    fn oam_stat_source(&self) -> bool {
        if self.ly < VBLANK_LINE && self.line_cycles < MODE2_DOTS {
            return true;
        }
        if self.line_cycles < MODE2_STAT_EARLY {
            return false;
        }
        let next = self.next_ly();
        if next < VBLANK_LINE {
            return true;
        }
        self.cgb_silicon && self.ly == VBLANK_LINE - 1 && next == VBLANK_LINE
    }

    fn stat_conditions_met(&self, regs: &Registers) -> bool {
        if regs.lcdc & LCDC_ENABLE == 0 {
            return false;
        }
        let lyc_eq = self.ly == regs.lyc;
        let mode = self.mode as u8;
        (regs.stat & 0x40 != 0 && lyc_eq)
            || (regs.stat & 0x20 != 0 && self.oam_stat_source())
            || (regs.stat & 0x10 != 0 && mode == 1)
            || (regs.stat & 0x08 != 0 && mode == 0)
    }

    fn sync_stat_line(&mut self, regs: &Registers) -> bool {
        let now = self.stat_conditions_met(regs);
        let rising = !self.stat_line && now;
        self.stat_line = now;
        rising
    }

    pub fn read_stat(&self, regs: &Registers) -> u8 {
        let mut v = (regs.stat & STAT_WRITE_MASK) | 0x80;
        if self.ly == regs.lyc {
            v |= 0x04;
        }
        if regs.lcdc & LCDC_ENABLE != 0 {
            v |= self.mode as u8;
        }
        v
    }

    /// Advance timing. `mode3_len(ly)` is sampled when entering Mode 3.
    /// `on_hblank(ly)` is called when a visible line enters Mode 0.
    pub fn tick(
        &mut self,
        regs: &Registers,
        t_cycles: u32,
        mut mode3_len: impl FnMut(u8) -> u32,
        mut on_hblank: impl FnMut(u8),
    ) -> TimingEvents {
        let mut ev = TimingEvents::default();
        if regs.lcdc & LCDC_ENABLE == 0 || t_cycles == 0 {
            return ev;
        }

        let mut remaining = t_cycles;
        while remaining > 0 {
            let left_in_line = CYCLES_PER_LINE - self.line_cycles;
            let step = remaining.min(left_in_line);
            let start = self.line_cycles;
            let end = start + step;

            if self.ly < VBLANK_LINE {
                if start < MODE2_DOTS && end >= MODE2_DOTS {
                    self.line_cycles = MODE2_DOTS;
                    self.mode0_start = MODE2_DOTS.saturating_add(mode3_len(self.ly));
                    self.mode0_start = self.mode0_start.min(CYCLES_PER_LINE);
                    self.mode = PpuMode::PixelTransfer;
                    if self.sync_stat_line(regs) {
                        ev.stat = true;
                    }
                }
                let mode0_at = self.mode0_start;
                if start < mode0_at && end >= mode0_at {
                    self.line_cycles = mode0_at;
                    self.mode = PpuMode::HBlank;
                    if self.sync_stat_line(regs) {
                        ev.stat = true;
                    }
                    on_hblank(self.ly);
                }
            }

            // Early Mode 2 STAT: final M-cycle before a Mode 2 line (LY unchanged).
            if start < MODE2_STAT_EARLY && end >= MODE2_STAT_EARLY {
                self.line_cycles = MODE2_STAT_EARLY;
                if self.sync_stat_line(regs) {
                    ev.stat = true;
                }
            }

            self.line_cycles = end;
            remaining -= step;

            if self.line_cycles >= CYCLES_PER_LINE {
                self.line_cycles = 0;
                let prev = self.ly;
                self.ly = self.ly.wrapping_add(1);
                if self.ly >= LINES_PER_FRAME {
                    self.ly = 0;
                }
                if prev == VBLANK_LINE - 1 && self.ly == VBLANK_LINE {
                    ev.vblank = true;
                }
                self.mode0_start = MODE0_START;
                self.mode = self.compute_mode();
                if self.sync_stat_line(regs) {
                    ev.stat = true;
                }
            }
        }
        ev
    }
}

impl Default for Timing {
    fn default() -> Self {
        Self::new()
    }
}
