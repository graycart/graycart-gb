//! Boot / startup behavior (Phase 7E).
//!
//! # Modes
//!
//! - [`BootMode::Fast`] — jump straight to the known DMG post-boot state (`$0100`).
//! - [`BootMode::BootRom`] — power-on CPU at `$0000` with the boot-ROM overlay enabled
//!   by the caller; firmware runs in the normal frame loop (not a blocking boot phase).
//!
//! External Nintendo DMG boot firmware (256-byte image, `$FF50` disable) is supported
//! when the user supplies a cached image beside the executable (see `frontend/boot_rom`).

use crate::bus::Bus;
use crate::cpu::Cpu;
use crate::hw::{CgbExecutionMode, HardwareModel};
use crate::ppu::{BGP, BGP_AFTER_BOOT, LCDC, LCDC_AFTER_BOOT, SCY};

/// How the emulator reaches cartridge entry at `$0100`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootMode {
    /// Direct post-boot CPU/MMIO snapshot (debug / conformance / `--skip-boot`).
    Fast,
    /// Real boot-ROM overlay; CPU starts at power-on (`$0000`).
    BootRom,
}

impl BootMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Fast => "skip (post-boot state)",
            Self::BootRom => "boot ROM",
        }
    }
}

fn apply_fast_shared_io(bus: &mut Bus) {
    bus.write8(SCY, 0);
    bus.write8(BGP, BGP_AFTER_BOOT);
    bus.write8(LCDC, LCDC_AFTER_BOOT);
    bus.apu.apply_after_boot();
    // Pan Docs power-up IF at `$0100` is `$E1` (VBlank still pending).
    bus.write8(0xFF0F, 0xE1);
}

fn apply_fast_dmg(cpu: &mut Cpu, bus: &mut Bus) {
    *cpu = Cpu::after_boot();
    apply_fast_shared_io(bus);
}

fn apply_fast_native_cgb(cpu: &mut Cpu, bus: &mut Bus) {
    *cpu = Cpu::after_boot_cgb();
    apply_fast_shared_io(bus);
    bus.ppu.cram.fill_bg_white();
    let cgb_byte = bus.cartridge.header.cgb_byte;
    bus.key0_mut().set_and_lock(cgb_byte);
    bus.set_opri(0);
    // CGB boot leaves both P1 select lines high → read `$FF` (`boot_hwio-C`).
    bus.write8(0xFF00, 0x30);
    bus.ppu.cram.write_bgpi(0xC8);
    bus.ppu.cram.write_obpi(0xD0);
}

fn apply_fast_cgb_compat(cpu: &mut Cpu, bus: &mut Bus) {
    *cpu = Cpu::after_boot_cgb_compat();
    apply_fast_shared_io(bus);
    bus.ppu.cram.fill_compat_gray_ramp();
    bus.key0_mut().set_and_lock(0x04);
    bus.set_opri(1);
    bus.write8(0xFF00, 0x30);
    bus.ppu.cram.write_bgpi(0xC8);
    bus.ppu.cram.write_obpi(0xD0);
}

/// Apply post-boot CPU + LCD + APU handoff for the bus hardware model.
///
/// APU defaults from [Pan Docs — Power Up Sequence](https://gbdev.io/pandocs/Power_Up_Sequence.html)
/// (NR50=`$77`, NR51=`$F3`, NR52=`$F1`, …). Fast never maps boot firmware.
pub fn apply_fast(cpu: &mut Cpu, bus: &mut Bus) {
    match bus.hardware_model() {
        HardwareModel::Dmg => apply_fast_dmg(cpu, bus),
        HardwareModel::Cgb {
            mode: CgbExecutionMode::NativeCgb,
        } => apply_fast_native_cgb(cpu, bus),
        HardwareModel::Cgb {
            mode: CgbExecutionMode::DmgCompatibility,
        } => apply_fast_cgb_compat(cpu, bus),
    }
}

/// Prepare CPU and bus for the selected boot path before the frame loop runs.
///
/// `BootMode::BootRom` resets the CPU to power-on state only; the caller must enable
/// the boot-ROM overlay on the bus before invoking this (and before `Cpu::new()` in
/// full-machine resets).
pub fn prepare_boot(mode: BootMode, cpu: &mut Cpu, bus: &mut Bus) {
    match mode {
        BootMode::Fast => apply_fast(cpu, bus),
        BootMode::BootRom => *cpu = Cpu::new(),
    }
}

/// Run the selected boot path. `on_frame` is retained for API compatibility but ignored.
pub fn run(
    mode: BootMode,
    cpu: &mut Cpu,
    bus: &mut Bus,
    _on_frame: impl FnMut(&crate::ppu::Framebuffer),
) {
    prepare_boot(mode, cpu, bus);
}

#[cfg(test)]
mod tests;
