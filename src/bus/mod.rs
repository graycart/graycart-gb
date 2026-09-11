mod boot;
mod io;
mod oam_dma;
mod serial;
mod vram_dma;
pub mod wram;

use crate::apu::{Apu, PCM12, PCM34};
use crate::cart::Cartridge;
use crate::cpu::Interrupt;
use crate::debug::TickProfile;
use crate::hw::{
    CgbScratch, ClockState, FF72, FF73, FF74, FF75, HDMA1, HDMA2, HDMA3, HDMA4, HDMA5,
    HardwareModel, KEY0, KEY1, Key0, Key1, STOP_PAUSE_CPU_T, VramDma, VramDmaMode,
};
use crate::input::{GameBoyButton, Joypad, P1};
use crate::ppu::{BGPD, BGPI, OBPD, OBPI, OPRI, Ppu, VBK};
use crate::timer::Timer;
use boot::BootRom;
use serial::SerialCapture;
use std::time::Instant;
use wram::{SVBK, Wram};

/// Game Boy memory map.
///
/// Mapped:
/// - `$0000`–`$7FFF` / `$A000`–`$BFFF` → [`Cartridge`] (MBC + external RAM)
/// - `$8000`–`$9FFF` VRAM (CPU locked in Mode 3 while LCD on)
/// - `$C000`–`$DFFF` WRAM
/// - `$FE00`–`$FE9F` → OAM (via [`Ppu`])
/// - `$FF00` → [`Joypad`] (P1)
/// - `$FF04`–`$FF07` → [`Timer`]
/// - `$FF10`–`$FF26` → [`Apu`]
/// - `$FF40`–`$FF4B` → [`Ppu`] LCDC/STAT/scroll/LY/LYC/DMA/BGP/OBP/WY/WX
/// - `$FF00`–`$FF7F` I/O register storage (remaining stubs)
/// - `$FF80`–`$FFFE` HRAM
/// - `$FFFF` IE
/// - `$FF50` BANK — disable boot ROM overlay (DMG and CGB)
const BOOT_DISABLE: u16 = 0xFF50;

pub struct Bus {
    pub cartridge: Cartridge,
    pub timer: Timer,
    pub ppu: Ppu,
    pub apu: Apu,
    pub joypad: Joypad,
    wram: Wram,
    io: [u8; 0x80],
    hram: [u8; 0x7F],
    ie: u8,
    /// Bytes written to `$FF01` (SB) — used by Blargg / Mooneye serial oracles.
    serial: SerialCapture,
    /// T-cycles of upcoming [`Self::tick`] that must not advance OAM DMA.
    ///
    /// A `$FF46` write is the last M-cycle of its instruction; the DMA start
    /// delay begins on the *following* instruction (Mooneye / Gekkio). Our
    /// step model still ticks after the write — suppress that last M-cycle (4 T) only.
    oam_dma_suppress_t: u32,
    /// When true, [`Self::tick`] accumulates into [`Self::tick_profile`].
    tick_profiling: bool,
    /// Last-frame tick breakdown (reset by the frontend each frame).
    pub tick_profile: TickProfile,
    /// Optional boot firmware overlay (DMG 256 or CGB 2048). Fast skip never maps this.
    boot: BootRom,
    /// CGB KEY0 (`$FF4C`). Locked on `$FF50` unmap / Fast. Inert on DMG.
    key0: Key0,
    /// CGB OPRI (`$FF6C`). Visible on CGB silicon (native and compat).
    opri: u8,
    /// Launch silicon. Selects CGB MMIO (VBK/SVBK/CRAM) when `Cgb { .. }`.
    hardware_model: HardwareModel,
    /// CGB KEY1/SPD. Speed is only this register — do not store a second bool.
    key1: Key1,
    /// Odd CPU T leftover when converting Double-speed CPU T → PPU/APU T (0 or 1).
    fixed_t_leftover: u8,
    /// Native CGB LCD VRAM DMA (`$FF51–$FF55`). Not OAM DMA.
    vram_dma: VramDma,
    /// Native CGB undocumented scratch `$FF72–$FF75`.
    cgb_scratch: CgbScratch,
    /// CPU HALT; HDMA HBlank bursts skip while the CPU is halted.
    cpu_halted: bool,
    /// Nested [`Self::tick`] from GDMA must not run HBlank HDMA bursts.
    gdma_running: bool,
    /// Nested [`Self::tick`] from an HDMA burst stall must not start another burst.
    hdma_burst: bool,
}

impl Bus {
    pub fn new(cartridge: Cartridge) -> Self {
        Self {
            cartridge,
            timer: Timer::new(),
            ppu: Ppu::new(),
            apu: Apu::new(),
            joypad: Joypad::new(),
            wram: Wram::new(),
            io: [0; 0x80],
            hram: [0; 0x7F],
            ie: 0,
            serial: SerialCapture::new(),
            oam_dma_suppress_t: 0,
            tick_profiling: false,
            tick_profile: TickProfile::default(),
            boot: BootRom::new(),
            key0: Key0::new(),
            opri: 0,
            hardware_model: HardwareModel::Dmg,
            key1: Key1::new(),
            fixed_t_leftover: 0,
            vram_dma: VramDma::new(),
            cgb_scratch: CgbScratch::new(),
            cpu_halted: false,
            gdma_running: false,
            hdma_burst: false,
        }
    }

    /// HDMA skips HBlank bursts while the CPU is halted.
    pub fn set_cpu_halted(&mut self, halted: bool) {
        self.cpu_halted = halted;
    }

    /// Last value from [`Self::set_cpu_halted`].
    pub fn cpu_halted(&self) -> bool {
        self.cpu_halted
    }

    pub fn hardware_model(&self) -> HardwareModel {
        self.hardware_model
    }

    /// Test/debug override of launch silicon.
    pub fn with_hardware_model(mut self, model: HardwareModel) -> Self {
        self.hardware_model = model;
        self.ppu.set_cgb_bg_attrs(model.native_cgb());
        self.ppu.set_cgb_silicon(model.cgb_mmio());
        self
    }

    /// Map a 256-byte DMG boot ROM over `$0000`–`$00FF` until `$FF50` or [`Self::disable_boot_rom`].
    pub fn enable_boot_rom(&mut self, rom: &[u8; 256]) {
        self.boot.enable_dmg(rom);
    }

    /// Map 2048-byte CGB firmware (header window `$0100–$01FF` stays cartridge).
    pub fn enable_cgb_boot_rom(&mut self, rom: &[u8; crate::hw::CGB_BOOT_ROM_SIZE]) {
        self.boot.enable_cgb(rom);
    }

    /// Force the boot overlay off (restore / snapshot paths).
    pub fn disable_boot_rom(&mut self) {
        self.boot.clear();
    }

    /// True while the boot ROM overlay is mapped for CPU reads.
    pub fn boot_rom_active(&self) -> bool {
        self.boot.is_mapped()
    }

    pub fn key0(&self) -> Key0 {
        self.key0
    }

    pub(crate) fn key0_mut(&mut self) -> &mut Key0 {
        &mut self.key0
    }

    pub fn opri(&self) -> u8 {
        self.opri
    }

    pub fn set_opri(&mut self, value: u8) {
        self.opri = value;
    }

    /// Power-on reset of volatile machine state; preserve cartridge SRAM and RTC.
    pub fn power_on_keep_battery(&mut self) {
        self.timer.power_on_reset();
        self.ppu.power_on_reset();
        self.apu.power_on_reset();
        self.joypad.power_on_reset();
        self.ppu.vram.fill(0);
        self.wram.power_on_reset();
        self.io.fill(0);
        self.hram.fill(0);
        self.ie = 0;
        self.serial.clear();
        self.oam_dma_suppress_t = 0;
        self.cartridge.power_on_reset_mapper();
        self.boot.clear();
        self.key0 = Key0::new();
        self.opri = 0;
        self.key1 = Key1::new();
        self.fixed_t_leftover = 0;
        self.vram_dma = VramDma::new();
        self.cpu_halted = false;
        self.gdma_running = false;
        self.hdma_burst = false;
    }

    /// Enable/disable Instant accumulation inside [`Self::tick`] (Debug Monitor).
    pub fn set_tick_profiling(&mut self, enabled: bool) {
        self.tick_profiling = enabled;
    }

    pub fn take_tick_profile(&mut self) -> TickProfile {
        let p = self.tick_profile;
        self.tick_profile.reset();
        p
    }

    /// Host frontend: button down. May request joypad IF.4.
    pub fn press_button(&mut self, button: GameBoyButton) {
        if self.joypad.press(button) {
            self.request_joypad_interrupt();
        }
    }

    /// Host frontend: button up.
    pub fn release_button(&mut self, button: GameBoyButton) {
        if self.joypad.release(button) {
            self.request_joypad_interrupt();
        }
    }

    /// Sync joypad pressed mask to host holds after load-state (no IF edges).
    pub fn sync_button_mask(&mut self, mask: u8) {
        self.joypad.set_pressed_mask(mask);
    }

    fn request_joypad_interrupt(&mut self) {
        let if_reg = self.read8(0xFF0F);
        self.write8(0xFF0F, if_reg | Interrupt::Joypad.mask());
    }

    /// Bytes captured from `$FF01` writes (conformance serial output).
    pub fn serial_output(&self) -> &[u8] {
        self.serial.as_slice()
    }

    /// UTF-8 lossy view of captured serial output.
    pub fn serial_text(&self) -> String {
        self.serial.text()
    }

    /// ROM-only bus for unit tests (flat mapping, no MBC / cart RAM).
    pub fn from_rom(rom: Vec<u8>) -> Self {
        Self::new(Cartridge::rom_only(rom))
    }

    /// Advance hardware by `t_cycles` CPU clocks.
    ///
    /// Timer, OAM DMA, and cartridge use CPU T. PPU and APU use fixed-dot T
    /// (`ClockState::cpu_t_to_fixed_t`); never a global `cpu_t * 2`.
    ///
    /// Active HDMA is stepped in 4 T slices so each Mode 0 enter copies one
    /// block at that HBlank instead of clumping after a bulk PPU tick.
    pub fn tick(&mut self, t_cycles: u32) {
        if t_cycles == 0 {
            return;
        }
        let should_slice_hdma =
            self.vram_dma.mode() == VramDmaMode::Hdma && !self.gdma_running && !self.hdma_burst;
        if should_slice_hdma && t_cycles > 4 {
            let mut left = t_cycles;
            while left > 0 {
                let step = left.min(4);
                self.tick_hardware(step);
                left -= step;
            }
            return;
        }
        self.tick_hardware(t_cycles);
    }

    fn tick_hardware(&mut self, t_cycles: u32) {
        let profiling = self.tick_profiling;

        let t0 = Instant::now();
        let timer_irq = self.timer.tick(t_cycles);
        if profiling {
            self.tick_profile.timer += t0.elapsed();
        }
        if timer_irq {
            let if_reg = self.read8(0xFF0F);
            self.write8(0xFF0F, if_reg | Interrupt::Timer.mask());
        }

        let mut dma_t = t_cycles;
        if self.oam_dma_suppress_t > 0 {
            let skip = dma_t.min(self.oam_dma_suppress_t);
            self.oam_dma_suppress_t -= skip;
            dma_t -= skip;
        }
        if dma_t > 0 {
            let t0 = Instant::now();
            self.advance_oam_dma(dma_t);
            if profiling {
                self.tick_profile.dma += t0.elapsed();
            }
        }

        let t0 = Instant::now();
        self.cartridge.tick(t_cycles);
        if profiling {
            self.tick_profile.cart += t0.elapsed();
        }

        let clock = self.clock_state();
        let fixed = clock.cpu_t_to_fixed_t(t_cycles, &mut self.fixed_t_leftover);
        if fixed == 0 {
            return;
        }

        let t0 = Instant::now();
        self.apu.tick(fixed);
        if profiling {
            self.tick_profile.apu += t0.elapsed();
        }

        let t0 = Instant::now();
        let ppu_irqs = self.ppu.tick(fixed);
        if profiling {
            self.tick_profile.ppu += t0.elapsed();
        }
        if ppu_irqs.vblank || ppu_irqs.stat {
            let if_reg = self.read8(0xFF0F);
            let mut next = if_reg;
            if ppu_irqs.vblank {
                next |= Interrupt::VBlank.mask();
            }
            if ppu_irqs.stat {
                next |= Interrupt::LcdStat.mask();
            }
            self.write8(0xFF0F, next);
        }

        if !self.gdma_running && !self.hdma_burst {
            self.run_hdma_hblank_bursts(&ppu_irqs.hblank_lines);
        }
    }

    pub(super) fn clock_state(&self) -> ClockState {
        if self.key1.current_double() {
            ClockState::Double
        } else {
            ClockState::Normal
        }
    }

    pub(super) fn cgb_memory(&self) -> bool {
        self.hardware_model.cgb_mmio()
    }

    pub(super) fn echo_wram_addr(addr: u16) -> u16 {
        0xC000 | ((addr - 0xE000) & 0x1FFF)
    }

    fn read8_wram_echo(&self, addr: u16) -> u8 {
        self.wram
            .read(Self::echo_wram_addr(addr), self.cgb_memory())
    }

    fn write8_wram_echo(&mut self, addr: u16, value: u8) {
        let cgb = self.cgb_memory();
        self.wram.write(Self::echo_wram_addr(addr), value, cgb);
    }

    pub fn read8(&self, addr: u16) -> u8 {
        if addr == P1 {
            return self.joypad.read();
        }
        if let Some(v) = self.timer.read(addr) {
            return v;
        }
        if let Some(v) = self.apu.read(addr) {
            return v;
        }
        if let Some(v) = self.ppu.read(addr) {
            return v;
        }
        // Bus conflict: CPU sees the byte DMA is currently transferring.
        if self.dma_conflicts_with(addr) {
            return self.ppu.dma_data_byte();
        }
        if let Some(b) = self.boot.overlay_byte(addr) {
            return b;
        }
        match addr {
            0x0000..=0x7FFF | 0xA000..=0xBFFF => self.cartridge.read8(addr),
            0x8000..=0x9FFF => {
                if !self.ppu.vram_cpu_accessible() {
                    return 0xFF;
                }
                self.ppu.vram.cpu_read(addr, self.cgb_memory())
            }
            0xC000..=0xDFFF => self.wram.read(addr, self.cgb_memory()),
            // Echo RAM / mirror of WRAM (needed for Mooneye oam_dma_start at $FDFF).
            0xE000..=0xFDFF => self.read8_wram_echo(addr),
            0xFF00..=0xFF7F => {
                if self.cgb_memory() {
                    let accessible = self.ppu.vram_cpu_accessible();
                    match addr {
                        BGPI => return self.ppu.cram.read_bgpi(),
                        // BGPD/OBPD are CGB Mode only. unused_hwio-C (CGB silicon,
                        // cart $0143=$00) treats $FF69/$FF6B as unused IO (write
                        // ignored, read $FF). BGPI/OBPI stay mapped (boot_hwio-C
                        // $C8/$D0; unused bit 6).
                        BGPD if self.hardware_model.native_cgb() => {
                            return self.ppu.cram.read_bgpd(accessible);
                        }
                        OBPI => return self.ppu.cram.read_obpi(),
                        OBPD if self.hardware_model.native_cgb() => {
                            return self.ppu.cram.read_obpd(accessible);
                        }
                        KEY0 if self.hardware_model.native_cgb() => return self.key0.read(),
                        KEY1 if self.hardware_model.native_cgb() => return self.key1.read(),
                        OPRI if self.hardware_model.native_cgb() => return self.opri,
                        FF72 | FF73 | FF75 => {
                            return self.cgb_scratch.read(addr).unwrap_or(0xFF);
                        }
                        FF74 => {
                            return if self.hardware_model.native_cgb() {
                                self.cgb_scratch.read(FF74).unwrap_or(0xFF)
                            } else {
                                crate::hw::ff74_locked()
                            };
                        }
                        PCM12 => {
                            return if self.hardware_model.native_cgb() {
                                self.apu.pcm12()
                            } else {
                                // Pan Docs: PCM12/PCM34 are CGB Mode only. On CGB
                                // silicon in Non-CGB mode they are not open bus
                                // ($FF) and not live generators — boot_hwio-C
                                // $FF76=$00 and unused_hwio-C write $FF→read $00.
                                0x00
                            };
                        }
                        PCM34 => {
                            return if self.hardware_model.native_cgb() {
                                self.apu.pcm34()
                            } else {
                                0x00
                            };
                        }
                        _ => {}
                    }
                }
                if self.hardware_model.native_cgb() {
                    match addr {
                        HDMA1 | HDMA2 | HDMA3 | HDMA4 => return 0xFF,
                        HDMA5 => return self.vram_dma.hdma5_read(),
                        _ => {}
                    }
                }
                if io::is_unmapped(addr) {
                    return 0xFF;
                }
                if addr == BOOT_DISABLE {
                    return 0xFF;
                }
                if addr == VBK {
                    return if self.cgb_memory() {
                        self.ppu.vram.read_vbk()
                    } else {
                        0xFF
                    };
                }
                if addr == SVBK {
                    return if self.hardware_model.native_cgb() {
                        self.wram.read_svbk()
                    } else {
                        0xFF
                    };
                }
                let raw = self.io[(addr - 0xFF00) as usize];
                io::apply_unused_bits(addr, raw)
            }
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize],
            0xFFFF => self.ie,
            _ => 0xFF,
        }
    }

    /// Armed CGB `STOP`: toggle KEY1 speed, reset leftover, freeze DIV, pause
    /// [`STOP_PAUSE_CPU_T`] CPU T (covers the STOP instruction budget).
    ///
    /// Clock is already the **new** speed, so PPU/APU see
    /// `cpu_t_to_fixed_t(8200)` at that rate (4100 fixed T when switching to Double).
    /// Returns `true` if a switch ran.
    pub fn on_stop(&mut self) -> bool {
        if !self.cgb_memory() {
            return false;
        }
        if !self.key1.try_switch_on_stop() {
            return false;
        }
        self.fixed_t_leftover = 0;
        let _ = self.timer.write(crate::timer::DIV, 0);
        self.timer.freeze_div(true);
        self.tick(STOP_PAUSE_CPU_T);
        self.timer.freeze_div(false);
        true
    }

    pub fn write8(&mut self, addr: u16, value: u8) {
        if addr == P1 {
            if self.joypad.write(value) {
                self.request_joypad_interrupt();
            }
            return;
        }
        if self.timer.write(addr, value) {
            return;
        }
        if self.apu.write(addr, value) {
            return;
        }
        // DMA write: start transfer, then suppress this instruction's post M-cycle.
        if addr == crate::ppu::DMA {
            let _ = self.ppu.write(addr, value);
            self.oam_dma_suppress_t = 4;
            return;
        }
        if self.ppu.write(addr, value) {
            return;
        }
        // Writes to the conflicting bus are ignored while DMA owns it.
        if self.dma_conflicts_with(addr) {
            return;
        }
        if addr == BOOT_DISABLE {
            self.io[(BOOT_DISABLE - 0xFF00) as usize] = value;
            if value != 0 {
                self.boot.unmap();
                if self.cgb_memory() {
                    self.key0.lock();
                }
            }
            return;
        }
        match addr {
            0x0000..=0x7FFF | 0xA000..=0xBFFF => self.cartridge.write8(addr, value),
            0x8000..=0x9FFF => {
                if self.ppu.vram_cpu_accessible() {
                    self.ppu.vram.cpu_write(addr, value, self.cgb_memory());
                }
            }
            0xC000..=0xDFFF => self.wram.write(addr, value, self.cgb_memory()),
            0xE000..=0xFDFF => self.write8_wram_echo(addr, value),
            0xFF00..=0xFF7F => {
                if self.cgb_memory() {
                    match addr {
                        BGPI => {
                            self.ppu.cram.write_bgpi(value);
                            return;
                        }
                        BGPD if self.hardware_model.native_cgb() => {
                            let accessible = self.ppu.vram_cpu_accessible();
                            self.ppu.cram.write_bgpd(value, accessible);
                            return;
                        }
                        OBPI => {
                            self.ppu.cram.write_obpi(value);
                            return;
                        }
                        OBPD if self.hardware_model.native_cgb() => {
                            let accessible = self.ppu.vram_cpu_accessible();
                            self.ppu.cram.write_obpd(value, accessible);
                            return;
                        }
                        KEY0 if self.hardware_model.native_cgb() => {
                            self.key0.write(value);
                            return;
                        }
                        KEY1 if self.hardware_model.native_cgb() => {
                            self.key1.write(value);
                            return;
                        }
                        OPRI if self.hardware_model.native_cgb() => {
                            self.opri = value;
                            return;
                        }
                        FF72 | FF73 | FF75 => {
                            self.cgb_scratch.write(addr, value);
                            return;
                        }
                        FF74 => {
                            if self.hardware_model.native_cgb() {
                                self.cgb_scratch.write(FF74, value);
                            }
                            return;
                        }
                        PCM12 | PCM34 => return,
                        _ => {}
                    }
                }
                if self.hardware_model.native_cgb() {
                    match addr {
                        HDMA1 => {
                            self.vram_dma.write_src_high(value);
                            return;
                        }
                        HDMA2 => {
                            self.vram_dma.write_src_low(value);
                            return;
                        }
                        HDMA3 => {
                            self.vram_dma.write_dest_high(value);
                            return;
                        }
                        HDMA4 => {
                            self.vram_dma.write_dest_low(value);
                            return;
                        }
                        HDMA5 => {
                            self.start_vram_dma(value);
                            return;
                        }
                        _ => {}
                    }
                }
                if io::is_unmapped(addr) {
                    return;
                }
                if addr == VBK {
                    if self.cgb_memory() {
                        self.ppu.vram.write_vbk(value);
                    }
                    return;
                }
                if addr == SVBK {
                    if self.hardware_model.native_cgb() {
                        self.wram.write_svbk(value);
                    }
                    return;
                }
                if addr == 0xFF01 {
                    self.serial.push(value);
                }
                let stored = if addr == 0xFF0F {
                    value & 0x1F
                } else if addr == 0xFF02 && value & 0x80 != 0 {
                    // Instant serial transfer complete for Blargg (clear SC.7).
                    value & 0x7F
                } else {
                    value
                };
                self.io[(addr - 0xFF00) as usize] = stored;
            }
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize] = value,
            0xFFFF => self.ie = value,
            _ => {}
        }
    }

    pub fn read16(&self, addr: u16) -> u16 {
        let lo = self.read8(addr);
        let hi = self.read8(addr.wrapping_add(1));
        u16::from_le_bytes([lo, hi])
    }

    pub fn write16(&mut self, addr: u16, value: u16) {
        let [lo, hi] = value.to_le_bytes();
        self.write8(addr, lo);
        self.write8(addr.wrapping_add(1), hi);
    }

    pub(crate) fn snapshot_ram(&self) -> crate::snapshot::BusRamStateV1 {
        crate::snapshot::BusRamStateV1 {
            vram: *self.ppu.vram.bank0(),
            wram: self.wram.snapshot_8k(self.cgb_memory()),
            io: self.io,
            hram: self.hram,
            ie: self.ie,
            serial: self.serial.as_slice().to_vec(),
            oam_dma_suppress_t: self.oam_dma_suppress_t,
        }
    }

    pub(crate) fn apply_snapshot_ram(&mut self, state: &crate::snapshot::BusRamStateV1) {
        *self.ppu.vram.bank0_mut() = state.vram;
        self.wram.apply_snapshot_8k(&state.wram, self.cgb_memory());
        self.io = state.io;
        self.hram = state.hram;
        self.ie = state.ie;
        self.serial = SerialCapture::from_vec(state.serial.clone());
        self.oam_dma_suppress_t = state.oam_dma_suppress_t;
    }
}

#[cfg(test)]
mod tests;
