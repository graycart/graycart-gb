//! Picture Processing Unit ([Pan Docs](https://gbdev.io/pandocs/LCDC.html)).
//!
//! Phase 4: timing, LCDC, STAT/modes/LYC, VBlank/STAT IRQs.
//! Phase 5A–5F: framebuffer, tiles, BG, window, OAM sprites, OAM DMA.

mod access;
mod attr;
mod background;
mod compose;
mod cram;
mod dma;
mod framebuffer;
mod mode3;
mod oam;
mod obj_attr;
mod opri;
mod pixel;
mod priority;
mod registers;
mod sprites;
mod tile;
mod timing;
mod vram;
mod window;

pub use access::{cpu_can_access_oam, cpu_can_access_vram};
pub use attr::{BgAttr, shade_from_rgb555};
pub use cram::{BGPD, BGPI, Cram, OBPD, OBPI};
pub use dma::OamDma;
pub use framebuffer::{Framebuffer, SCREEN_HEIGHT, SCREEN_WIDTH, Shade, shade_from_bgp};
pub use mode3::{MODE3_BASE, mode3_length};
pub use oam::{OAM_BASE, OAM_SIZE, Oam};
pub use obj_attr::ObjAttr;
pub use opri::OPRI;
pub use registers::{
    BGP, BGP_AFTER_BOOT, DMA, LCDC, LCDC_AFTER_BOOT, LCDC_POWER_ON, LY, LYC, OBP0, OBP1, SCX, SCY,
    STAT, WX, WY,
};
pub use tile::{decode_tile_row, tile_addr_8000, tile_addr_8800, tile_row_offset};
pub use timing::{
    CYCLES_PER_LINE, LINES_PER_FRAME, MODE0_START, MODE2_DOTS, MODE2_STAT_EARLY, MODE3_DOTS,
    PpuMode, VBLANK_LINE,
};
pub use vram::{VBK, Vram};

use background::render_scanline as render_background;
use compose::mix_cgb_line;
use framebuffer::Shade as FbShade;
use pixel::CgbPixel;
use registers::{LCDC_BG_ENABLE, Registers};
use timing::{PpuMode as Mode, Timing};
use window::render_scanline as render_window;

/// Interrupt requests produced by one [`Ppu::tick`] call.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PpuIrqs {
    pub vblank: bool,
    pub stat: bool,
    /// Visible-line LYs that entered Mode 0 during this tick (HDMA).
    pub hblank_lines: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct Ppu {
    regs: Registers,
    timing: Timing,
    pub framebuffer: Framebuffer,
    frame_ready: bool,
    window_line: u8,
    window_y_active: bool,
    pub oam: Oam,
    pub vram: Vram,
    pub cram: Cram,
    dma: OamDma,
    /// Native CGB: BG/window attributes from VRAM bank 1 + CRAM shades.
    cgb_bg_attrs: bool,
    /// CGB silicon (Native or DMG-compat): Mode 2 STAT at LY=144.
    cgb_silicon: bool,
}

impl Default for Ppu {
    fn default() -> Self {
        Self::new()
    }
}

impl Ppu {
    pub fn new() -> Self {
        let regs = Registers::new();
        let mut timing = Timing::new();
        timing.sync_stat_from_regs(&regs);
        Self {
            regs,
            timing,
            framebuffer: Framebuffer::new(),
            frame_ready: false,
            window_line: 0,
            window_y_active: false,
            oam: Oam::new(),
            vram: Vram::new(),
            cram: Cram::new(),
            dma: OamDma::new(),
            cgb_bg_attrs: false,
            cgb_silicon: false,
        }
    }

    /// Power-on reset (same defaults as [`Self::new`]).
    ///
    /// LCDC is [`LCDC_POWER_ON`] (`$00`, LCD off). Fast boot writes [`LCDC_AFTER_BOOT`].
    /// Launch-time [`Self::set_cgb_bg_attrs`] / [`Self::set_cgb_silicon`] are preserved.
    pub fn power_on_reset(&mut self) {
        let cgb_bg_attrs = self.cgb_bg_attrs;
        let cgb_silicon = self.cgb_silicon;
        *self = Self::new();
        self.set_cgb_bg_attrs(cgb_bg_attrs);
        self.set_cgb_silicon(cgb_silicon);
    }

    /// Enable CGB BG/window attributes (`NativeCgb` only; DMG and DMG-compat stay off).
    pub fn set_cgb_bg_attrs(&mut self, enabled: bool) {
        self.cgb_bg_attrs = enabled;
        self.framebuffer.set_presents_cgb_color(enabled);
    }

    /// CGB silicon (including DMG-compat): Mode 2 STAT also fires entering LY=144.
    pub fn set_cgb_silicon(&mut self, enabled: bool) {
        self.cgb_silicon = enabled;
        self.timing.set_cgb_silicon(enabled);
    }

    pub fn cgb_bg_attrs(&self) -> bool {
        self.cgb_bg_attrs
    }

    pub fn cgb_silicon(&self) -> bool {
        self.cgb_silicon
    }

    pub fn ly(&self) -> u8 {
        self.timing.ly
    }

    pub fn lcdc(&self) -> u8 {
        self.regs.lcdc
    }

    pub fn scx(&self) -> u8 {
        self.regs.scx
    }

    pub fn scy(&self) -> u8 {
        self.regs.scy
    }

    pub fn bgp(&self) -> u8 {
        self.regs.bgp
    }

    pub fn wx(&self) -> u8 {
        self.regs.wx
    }

    pub fn wy(&self) -> u8 {
        self.regs.wy
    }

    pub fn window_line(&self) -> u8 {
        self.window_line
    }

    pub fn dma_active(&self) -> bool {
        self.dma.active()
    }

    pub fn dma_blocks_oam(&self) -> bool {
        self.dma.blocks_oam()
    }

    pub fn dma_source_page(&self) -> u8 {
        self.dma.page()
    }

    pub fn dma_data_byte(&self) -> u8 {
        self.dma.data_byte()
    }

    pub fn set_dma_data_byte(&mut self, value: u8) {
        self.dma.set_data_byte(value);
    }

    pub fn mode(&self) -> PpuMode {
        if self.regs.lcd_enabled() {
            self.timing.mode
        } else {
            Mode::HBlank
        }
    }

    pub fn lcd_enabled(&self) -> bool {
        self.regs.lcd_enabled()
    }

    /// CPU may access OAM (`$FE00`–`$FE9F`) in the current LCD/mode/DMA state.
    pub fn oam_cpu_accessible(&self) -> bool {
        if self.dma.blocks_oam() {
            return false;
        }
        access::cpu_can_access_oam(self.lcd_enabled(), self.mode())
    }

    /// CPU may access VRAM (`$8000`–`$9FFF`) in the current LCD/mode state.
    pub fn vram_cpu_accessible(&self) -> bool {
        access::cpu_can_access_vram(self.lcd_enabled(), self.mode())
    }

    pub fn line_cycles(&self) -> u32 {
        self.timing.line_cycles
    }

    /// Readable STAT (mode bits + coincidence + enables).
    pub fn stat(&self) -> u8 {
        self.timing.read_stat(&self.regs)
    }

    /// Dot where Mode 0 begins on the current line (after Mode 3 penalties).
    pub fn mode0_start(&self) -> u32 {
        self.timing.mode0_start
    }

    pub fn set_line_state(&mut self, ly: u8, line_cycles: u32, mode: PpuMode) {
        self.timing.ly = ly;
        self.timing.line_cycles = line_cycles;
        self.timing.mode = mode;
        // Keep Mode 0 boundary coherent for unit tests that poke line state.
        match mode {
            Mode::HBlank => {
                self.timing.mode0_start = line_cycles.min(MODE0_START);
            }
            Mode::PixelTransfer => {
                self.timing.mode0_start = MODE0_START.max(line_cycles + 1);
            }
            Mode::OamScan => {
                self.timing.mode0_start = MODE0_START;
            }
            Mode::VBlank => {}
        }
    }

    pub fn take_frame_ready(&mut self) -> bool {
        let ready = self.frame_ready;
        self.frame_ready = false;
        ready
    }

    fn reset_window_state(&mut self) {
        self.window_line = 0;
        self.window_y_active = false;
    }

    /// Start OAM DMA from `page << 8` (write to `$FF46`).
    pub fn start_dma(&mut self, page: u8) {
        self.dma.start(page);
    }

    /// Advance DMA; returns source addresses to copy into successive OAM bytes.
    pub fn tick_dma(&mut self, t_cycles: u32) -> Vec<u16> {
        self.dma.tick(t_cycles)
    }

    /// Write one DMA-fetched byte into OAM at the transfer’s current index−1.
    pub fn dma_write_oam_byte(&mut self, index: u8, value: u8) {
        self.oam.write_index(index as usize, value);
    }

    fn render_visible_line(&mut self, ly: u8) {
        if self.regs.wy == ly {
            self.window_y_active = true;
        }
        let lcdc = self.regs.lcdc;
        let bgp = self.regs.bgp;
        let scx = self.regs.scx;
        let scy = self.regs.scy;
        let wx = self.regs.wx;
        let y_active = self.window_y_active;
        let win_line = self.window_line;
        let obp0 = self.regs.obp0;
        let obp1 = self.regs.obp1;
        let mut colors = [0u8; SCREEN_WIDTH];
        let mut bg_priority = [false; SCREEN_WIDTH];
        let mut cgb_line = [CgbPixel::default(); SCREEN_WIDTH];
        let native = self.cgb_bg_attrs;
        let (vram1, cram) = if native {
            (Some(self.vram.bank(1).as_slice()), Some(&self.cram))
        } else {
            (None, None)
        };
        render_background(
            &mut background::BgLine {
                ly,
                lcdc,
                scx,
                scy,
                bgp,
                vram: self.vram.bank0(),
                vram1,
                cram,
                colors: &mut colors,
                bg_priority: if native { Some(&mut bg_priority) } else { None },
                cgb: if native { Some(&mut cgb_line) } else { None },
            },
            &mut self.framebuffer,
        );
        if render_window(
            &mut window::WindowLine {
                ly,
                lcdc,
                wx,
                y_active,
                win_line,
                bgp,
                vram: self.vram.bank0(),
                vram1,
                cram,
                colors: &mut colors,
                bg_priority: if native { Some(&mut bg_priority) } else { None },
                cgb: if native { Some(&mut cgb_line) } else { None },
            },
            &mut self.framebuffer,
        ) {
            self.window_line = win_line.wrapping_add(1);
        }
        if native {
            let lcdc0 = lcdc & LCDC_BG_ENABLE != 0;
            let sprite_line = sprites::SpriteLine {
                ly,
                lcdc,
                obp0,
                obp1,
                oam: &self.oam,
                vram: self.vram.bank0(),
                bg_colors: &colors,
                vram1: Some(self.vram.bank(1).as_slice()),
                cram: Some(&self.cram),
            };
            let collected = sprites::collect_cgb_obj_line(&sprite_line);
            let mixed = mix_cgb_line(&cgb_line, &collected, lcdc0);
            for (x, pixel) in mixed.iter().enumerate() {
                self.framebuffer.set_cgb_pixel(
                    x,
                    ly as usize,
                    shade_from_rgb555(pixel.rgb555),
                    pixel.rgb555,
                );
            }
        } else {
            sprites::render_scanline(
                &sprites::SpriteLine {
                    ly,
                    lcdc,
                    obp0,
                    obp1,
                    oam: &self.oam,
                    vram: self.vram.bank0(),
                    bg_colors: &colors,
                    vram1: None,
                    cram: None,
                },
                &mut self.framebuffer,
            );
        }
    }

    pub fn read(&self, addr: u16) -> Option<u8> {
        if let Some(v) = self.oam.read(addr) {
            return Some(if self.oam_cpu_accessible() { v } else { 0xFF });
        }
        match addr {
            LCDC => Some(self.regs.lcdc),
            STAT => Some(self.timing.read_stat(&self.regs)),
            SCY => Some(self.regs.scy),
            SCX => Some(self.regs.scx),
            LY => Some(self.timing.ly),
            LYC => Some(self.regs.lyc),
            DMA => Some(self.dma.last_value()),
            BGP => Some(self.regs.bgp),
            OBP0 => Some(self.regs.obp0),
            OBP1 => Some(self.regs.obp1),
            WY => Some(self.regs.wy),
            WX => Some(self.regs.wx),
            _ => None,
        }
    }

    pub fn write(&mut self, addr: u16, value: u8) -> bool {
        // Consume OAM addresses even when locked so the bus does not fall through.
        if (OAM_BASE..OAM_BASE + OAM_SIZE as u16).contains(&addr) {
            if self.oam_cpu_accessible() {
                let _ = self.oam.write(addr, value);
            }
            return true;
        }
        match addr {
            LCDC => {
                let was_on = self.regs.lcd_enabled();
                self.regs.lcdc = value;
                let now_on = self.regs.lcd_enabled();
                if was_on && !now_on {
                    self.timing.reset_line_state(Mode::HBlank);
                    self.timing.sync_stat_from_regs(&self.regs);
                    self.framebuffer.clear(FbShade::Lightest);
                    self.reset_window_state();
                } else if !was_on && now_on {
                    self.timing.reset_line_state(Mode::OamScan);
                    self.timing.sync_stat_from_regs(&self.regs);
                    self.reset_window_state();
                }
                true
            }
            STAT => {
                self.regs.write_stat_selects(value);
                self.timing.sync_stat_from_regs(&self.regs);
                true
            }
            SCY => {
                self.regs.scy = value;
                true
            }
            SCX => {
                self.regs.scx = value;
                true
            }
            LY => true,
            LYC => {
                self.regs.lyc = value;
                self.timing.sync_stat_from_regs(&self.regs);
                true
            }
            DMA => {
                self.start_dma(value);
                true
            }
            BGP => {
                self.regs.bgp = value;
                true
            }
            OBP0 => {
                self.regs.obp0 = value;
                true
            }
            OBP1 => {
                self.regs.obp1 = value;
                true
            }
            WY => {
                self.regs.wy = value;
                true
            }
            WX => {
                self.regs.wx = value;
                true
            }
            _ => false,
        }
    }

    /// Advance by `t_cycles`. Renders each visible scanline when it enters HBlank.
    pub fn tick(&mut self, t_cycles: u32) -> PpuIrqs {
        let mut lines = Vec::new();
        // Clone small state so Mode 3 length can be sampled at Mode 2→3 without
        // fighting the mutable borrow on `timing`.
        let regs = self.regs.clone();
        let oam = self.oam.clone();
        let window_y_active = self.window_y_active;
        let ev = self.timing.tick(
            &regs,
            t_cycles,
            |ly| mode3::mode3_length(ly, &regs, &oam, window_y_active || regs.wy == ly),
            |ly| lines.push(ly),
        );
        for ly in &lines {
            self.render_visible_line(*ly);
        }
        if ev.vblank {
            self.frame_ready = true;
            self.reset_window_state();
        }
        PpuIrqs {
            vblank: ev.vblank,
            stat: ev.stat,
            hblank_lines: lines,
        }
    }

    pub(crate) fn snapshot(&self) -> crate::snapshot::PpuStateV1 {
        let (dma, dma_active, dma_src, dma_index, dma_delay) = self.dma.snapshot();
        crate::snapshot::PpuStateV1 {
            lcdc: self.regs.lcdc,
            stat: self.regs.stat,
            scy: self.regs.scy,
            scx: self.regs.scx,
            ly: self.timing.ly,
            lyc: self.regs.lyc,
            dma,
            bgp: self.regs.bgp,
            obp0: self.regs.obp0,
            obp1: self.regs.obp1,
            wy: self.regs.wy,
            wx: self.regs.wx,
            mode: self.mode().as_u8(),
            dot: self.timing.line_cycles.min(u16::MAX as u32) as u16,
            window_line: self.window_line,
            window_y_active: self.window_y_active,
            oam: *self.oam.bytes(),
            dma_active,
            dma_src,
            dma_index,
            dma_delay,
        }
    }

    pub(crate) fn apply_snapshot(&mut self, state: &crate::snapshot::PpuStateV1) {
        self.regs.lcdc = state.lcdc;
        self.regs.stat = state.stat;
        self.regs.scy = state.scy;
        self.regs.scx = state.scx;
        self.regs.lyc = state.lyc;
        self.regs.bgp = state.bgp;
        self.regs.obp0 = state.obp0;
        self.regs.obp1 = state.obp1;
        self.regs.wy = state.wy;
        self.regs.wx = state.wx;
        self.timing.ly = state.ly;
        self.timing.line_cycles = u32::from(state.dot);
        self.timing.mode = Mode::from_u8(state.mode);
        self.timing.mode0_start = MODE0_START;
        self.timing.stat_line = false;
        self.timing.sync_stat_from_regs(&self.regs);
        self.window_line = state.window_line;
        self.window_y_active = state.window_y_active;
        for (i, &byte) in state.oam.iter().enumerate() {
            self.oam.write_index(i, byte);
        }
        self.dma.apply_snapshot(
            state.dma,
            state.dma_active,
            state.dma_src,
            state.dma_index,
            state.dma_delay,
        );
    }
}

#[cfg(test)]
mod tests;
