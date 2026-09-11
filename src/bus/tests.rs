use super::*;
use crate::cart::{Cartridge, Header};

const ROM_BANK_SIZE: usize = 0x4000;
use crate::cpu::{Cpu, Interrupt, interrupts, step};
use crate::ppu::{CYCLES_PER_LINE, LCDC, LCDC_AFTER_BOOT, LY};
use crate::timer::{DIV, TAC, TIMA, TMA};

const BOOT_DISABLE: u16 = 0xFF50;

fn mbc3_battery_cart() -> Cartridge {
    let mut rom = vec![0xFF; 4 * ROM_BANK_SIZE];
    rom[0x0000] = 0x10;
    rom[ROM_BANK_SIZE] = 0x11;
    rom[2 * ROM_BANK_SIZE] = 0x22;
    rom[0x0147] = 0x13; // MBC3+RAM+BATTERY
    rom[0x0148] = 0x01;
    rom[0x0149] = 0x02; // 8 KiB RAM
    let header = Header::parse(&rom).unwrap();
    Cartridge::from_parts(rom, header).unwrap()
}

fn mbc3_rtc_cart() -> Cartridge {
    let mut rom = vec![0xFF; 4 * ROM_BANK_SIZE];
    rom[0x0000] = 0x10;
    rom[ROM_BANK_SIZE] = 0x11;
    rom[2 * ROM_BANK_SIZE] = 0x22;
    rom[0x0147] = 0x10; // MBC3+TIMER+RAM+BATTERY
    rom[0x0148] = 0x01;
    rom[0x0149] = 0x02; // 8 KiB RAM
    let header = Header::parse(&rom).unwrap();
    Cartridge::from_parts(rom, header).unwrap()
}

fn enable_lcd(bus: &mut Bus) {
    bus.write8(LCDC, LCDC_AFTER_BOOT);
}

#[test]
fn rom_read_and_wram_round_trip() {
    let mut bus = Bus::from_rom(vec![0x00, 0xC3, 0x50, 0x01]);
    assert_eq!(bus.read8(0x0000), 0x00);
    assert_eq!(bus.read8(0x0001), 0xC3);

    bus.write8(0xC000, 0xAB);
    assert_eq!(bus.read8(0xC000), 0xAB);

    bus.write8(0xFF80, 0x12);
    assert_eq!(bus.read8(0xFF80), 0x12);
}

#[test]
fn rom_bank_writes_go_to_mbc_not_rom_bytes() {
    let mut bus = Bus::from_rom(vec![0x42]);
    bus.write8(0x0000, 0x99);
    assert_eq!(bus.read8(0x0000), 0x42);
}

#[test]
fn wram_cf1a_round_trip() {
    let mut bus = Bus::from_rom(vec![0; 0x200]);
    bus.write8(0xCF1A, 0x00);
    assert_eq!(bus.read8(0xCF1A), 0x00);
    bus.write8(0xCF1A, 0x5A);
    assert_eq!(bus.read8(0xCF1A), 0x5A);
}

#[test]
fn vram_round_trip() {
    let mut bus = Bus::from_rom(vec![0; 0x200]);
    // Mode 2 allows VRAM; keep LCD timing out of Mode 3.
    bus.ppu.set_line_state(0, 0, crate::ppu::PpuMode::OamScan);
    bus.write8(0x8000, 0x5A);
    bus.write8(0x9FFF, 0xA5);
    assert_eq!(bus.read8(0x8000), 0x5A);
    assert_eq!(bus.read8(0x9FFF), 0xA5);
}

#[test]
fn io_ie_and_real_ly() {
    let mut bus = Bus::from_rom(vec![0; 0x200]);
    bus.write8(0xFFFF, 0x0F);
    assert_eq!(bus.read8(0xFFFF), 0x0F);

    bus.write8(0xFF0F, 0x00);
    assert_eq!(bus.read8(0xFF0F), 0xE0);

    bus.write8(0xFF40, 0x80);
    assert_eq!(bus.read8(0xFF40), 0x80);

    assert_eq!(bus.read8(LY), 0);
    bus.tick(CYCLES_PER_LINE);
    assert_eq!(bus.read8(LY), 1);
}

#[test]
fn lcd_disable_stops_ly_and_vblank() {
    use crate::ppu::LCDC;

    let mut bus = Bus::from_rom(vec![0; 0x200]);
    bus.write8(LCDC, 0x00);
    assert_eq!(bus.read8(LY), 0);
    bus.tick(CYCLES_PER_LINE * 200);
    assert_eq!(bus.read8(LY), 0);
    assert_eq!(bus.read8(0xFF0F) & Interrupt::VBlank.mask(), 0);
}

#[test]
fn halted_step_still_advances_ppu() {
    let mut cpu = Cpu::after_boot();
    cpu.halted = true;
    let mut bus = Bus::from_rom(vec![0; 0x200]);
    enable_lcd(&mut bus);
    assert_eq!(bus.read8(LY), 0);
    // 456 T-cycles / 4 T per halt step
    for _ in 0..CYCLES_PER_LINE / 4 {
        step(&mut cpu, &mut bus).unwrap();
        assert!(cpu.halted);
    }
    assert_eq!(bus.read8(LY), 1);
}

#[test]
fn timer_overflow_sets_if_and_wakes_halted_cpu() {
    let mut cpu = Cpu::after_boot();
    cpu.pc = 0x1234;
    cpu.sp = 0xFFFE;
    cpu.halted = true;
    cpu.ime = true;

    let mut bus = Bus::from_rom(vec![0; 0x200]);
    bus.write8(0xFFFF, Interrupt::Timer.mask());
    bus.write8(TAC, 0b0000_0101); // enable, 262144 Hz
    bus.write8(TIMA, 0xFF);
    bus.write8(TMA, 0x00);
    bus.write8(0xFF0F, 0x00);

    // Overflow starts a 4 T-cycle reload delay before IF.2.
    bus.tick(16);
    assert_eq!(bus.read8(0xFF0F) & Interrupt::Timer.mask(), 0);
    bus.tick(4);
    assert_ne!(bus.read8(0xFF0F) & Interrupt::Timer.mask(), 0);

    assert!(interrupts::poll(&mut cpu, &mut bus));
    assert!(!cpu.halted);
    assert!(!cpu.ime);
    assert_eq!(cpu.pc, 0x0050);
    assert_eq!(cpu.sp, 0xFFFC);
}

#[test]
fn timer_overflow_through_step_while_halted() {
    let mut cpu = Cpu::after_boot();
    cpu.pc = 0x1234;
    cpu.sp = 0xFFFE;
    cpu.halted = true;
    cpu.ime = true;

    let mut bus = Bus::from_rom(vec![0; 0x200]);
    bus.write8(0xFFFF, Interrupt::Timer.mask());
    bus.write8(TAC, 0b0000_0101);
    bus.write8(TIMA, 0xFF);
    bus.write8(0xFF0F, 0x00);

    for _ in 0..16 {
        step(&mut cpu, &mut bus).unwrap();
        if !cpu.halted {
            break;
        }
    }
    assert!(!cpu.halted);
    assert_eq!(cpu.pc, 0x0050);
}

#[test]
fn vblank_sets_if_and_wakes_halted_cpu() {
    let mut cpu = Cpu::after_boot();
    cpu.pc = 0x1234;
    cpu.sp = 0xFFFE;
    cpu.halted = true;
    cpu.ime = true;

    let mut bus = Bus::from_rom(vec![0; 0x200]);
    enable_lcd(&mut bus);
    bus.write8(0xFFFF, Interrupt::VBlank.mask());
    bus.write8(0xFF0F, 0x00);
    bus.ppu
        .set_line_state(143, CYCLES_PER_LINE - 1, crate::ppu::PpuMode::HBlank);

    bus.tick(1);
    assert_eq!(bus.read8(LY), 144);
    assert_ne!(bus.read8(0xFF0F) & Interrupt::VBlank.mask(), 0);

    assert!(interrupts::poll(&mut cpu, &mut bus));
    assert!(!cpu.halted);
    assert_eq!(cpu.pc, 0x0040);
}

#[test]
fn stat_mode0_sets_if_through_bus() {
    use crate::ppu::{MODE0_START, STAT};

    let mut bus = Bus::from_rom(vec![0; 0x200]);
    enable_lcd(&mut bus);
    bus.write8(STAT, 0x08); // Mode 0
    bus.write8(0xFF0F, 0x00);
    bus.tick(MODE0_START);
    assert_ne!(bus.read8(0xFF0F) & Interrupt::LcdStat.mask(), 0);
    assert_eq!(bus.read8(STAT) & 0x03, 0);
}

#[test]
fn oam_round_trip_through_bus() {
    let mut bus = Bus::from_rom(vec![0; 0x200]);
    bus.ppu.set_line_state(0, 300, crate::ppu::PpuMode::HBlank);
    bus.write8(0xFE00, 0x12);
    bus.write8(0xFE9F, 0x34);
    assert_eq!(bus.read8(0xFE00), 0x12);
    assert_eq!(bus.read8(0xFE9F), 0x34);
}

#[test]
fn mode2_blocks_cpu_oam_allows_vram() {
    use crate::ppu::PpuMode;

    let mut bus = Bus::from_rom(vec![0; 0x200]);
    enable_lcd(&mut bus);
    bus.ppu.set_line_state(0, 300, PpuMode::HBlank);
    bus.write8(0xFE00, 0x55);
    bus.write8(0x8000, 0x66);

    bus.ppu.set_line_state(0, 40, PpuMode::OamScan);
    assert_eq!(bus.read8(0xFE00), 0xFF);
    assert_eq!(bus.read8(0x8000), 0x66);
    bus.write8(0xFE00, 0x99);
    bus.write8(0x8000, 0x77);
    assert_eq!(bus.read8(0x8000), 0x77);

    bus.ppu.set_line_state(0, 300, PpuMode::HBlank);
    assert_eq!(bus.read8(0xFE00), 0x55);
}

#[test]
fn mode3_blocks_cpu_oam_and_vram() {
    use crate::ppu::PpuMode;

    let mut bus = Bus::from_rom(vec![0; 0x200]);
    enable_lcd(&mut bus);
    bus.ppu.set_line_state(0, 300, PpuMode::HBlank);
    bus.write8(0xFE00, 0x11);
    bus.write8(0x8000, 0x22);

    bus.ppu.set_line_state(0, 100, PpuMode::PixelTransfer);
    assert_eq!(bus.read8(0xFE00), 0xFF);
    assert_eq!(bus.read8(0x8000), 0xFF);
    bus.write8(0xFE00, 0x33);
    bus.write8(0x8000, 0x44);

    bus.ppu.set_line_state(0, 300, PpuMode::HBlank);
    assert_eq!(bus.read8(0xFE00), 0x11);
    assert_eq!(bus.read8(0x8000), 0x22);
}

#[test]
fn mode0_and_vblank_allow_cpu_oam_and_vram() {
    use crate::ppu::PpuMode;

    let mut bus = Bus::from_rom(vec![0; 0x200]);
    for mode in [PpuMode::HBlank, PpuMode::VBlank] {
        bus.ppu.set_line_state(144, 0, mode);
        bus.write8(0xFE00, 0xAB);
        bus.write8(0x8000, 0xCD);
        assert_eq!(bus.read8(0xFE00), 0xAB, "{mode:?}");
        assert_eq!(bus.read8(0x8000), 0xCD, "{mode:?}");
    }
}

#[test]
fn lcd_off_allows_cpu_oam_and_vram_during_mode3() {
    use crate::ppu::{LCDC, PpuMode};

    let mut bus = Bus::from_rom(vec![0; 0x200]);
    bus.write8(LCDC, 0x00);
    bus.ppu.set_line_state(0, 100, PpuMode::PixelTransfer);
    bus.write8(0xFE00, 0x42);
    bus.write8(0x8000, 0x24);
    assert_eq!(bus.read8(0xFE00), 0x42);
    assert_eq!(bus.read8(0x8000), 0x24);
}

#[test]
fn apu_nr52_power_and_ch1_through_bus() {
    use crate::apu::{NR12, NR14, NR50, NR51, NR52};

    let mut bus = Bus::from_rom(vec![0; 0x200]);
    bus.write8(NR52, 0x80);
    assert_ne!(bus.read8(NR52) & 0x80, 0);
    bus.write8(NR50, 0x77);
    bus.write8(NR51, 0x11);
    bus.write8(NR12, 0xF0);
    bus.write8(NR14, 0x80);
    assert_ne!(bus.read8(NR52) & 0x01, 0);
    bus.tick(4_194_304 / 120);
    assert!(!bus.apu.take_samples().is_empty());
}

#[test]
fn wram_echo_mirrors_c000_for_cpu() {
    let mut bus = Bus::from_rom(vec![0; 0x200]);
    // $E000–$FDFF mirrors $C000–$DDFF (not $DFFF — that has no echo alias).
    bus.write8(0xDDFF, 0x77);
    assert_eq!(bus.read8(0xFDFF), 0x77);
    bus.write8(0xFDFE, 0x42);
    assert_eq!(bus.read8(0xDDFE), 0x42);
}

#[test]
fn oam_dma_start_delay_begins_after_write_m_cycle() {
    use crate::ppu::{DMA, PpuMode};

    let mut bus = Bus::from_rom(vec![0; 0x200]);
    bus.ppu.set_line_state(0, 300, PpuMode::HBlank);
    bus.write8(0xFE00, 0x04);
    bus.write8(DMA, 0x80);
    // Post-write M-cycle is suppressed for DMA — OAM still CPU-readable.
    bus.tick(4);
    assert!(!bus.ppu.dma_blocks_oam());
    assert_eq!(bus.read8(0xFE00), 0x04);
    // Next M-cycle: delay completes and transfer locks OAM.
    bus.tick(4);
    assert!(bus.ppu.dma_blocks_oam());
    assert_eq!(bus.read8(0xFE00), 0xFF);
}

#[test]
fn oam_dma_vram_bus_conflict_returns_in_flight_byte() {
    use crate::ppu::DMA;

    let mut bus = Bus::from_rom(vec![0; 0x200]);
    bus.write8(0x8000, 0xD7);
    bus.write8(0x8001, 0xAB);
    bus.write8(DMA, 0x80);
    // suppress 4 + delay 4 + first byte copy 4
    bus.tick(4 + 4 + 4);
    assert!(bus.ppu.dma_blocks_oam());
    assert_eq!(bus.ppu.dma_data_byte(), 0xD7);
    assert_eq!(bus.read8(0x8000), 0xD7);
    bus.tick(4);
    assert_eq!(bus.ppu.dma_data_byte(), 0xAB);
    assert_eq!(bus.read8(0x8001), 0xAB);
}

#[test]
fn oam_dma_copies_wram_into_oam() {
    use crate::ppu::{DMA, PpuMode};

    let mut bus = Bus::from_rom(vec![0; 0x200]);
    for i in 0u16..160 {
        bus.write8(0xC000 + i, (i as u8).wrapping_add(0x40));
    }
    bus.write8(DMA, 0xC0);
    assert!(bus.ppu.dma_active());
    // Write M-cycle suppress + start delay + 160 bytes.
    bus.tick(4 + 4 + 160 * 4);
    assert!(!bus.ppu.dma_active());
    // DMA bypasses CPU locks; verify via accessible mode for bus reads.
    bus.ppu.set_line_state(0, 300, PpuMode::HBlank);
    for i in 0u16..160 {
        assert_eq!(
            bus.read8(0xFE00 + i),
            (i as u8).wrapping_add(0x40),
            "oam byte {i}"
        );
    }
    assert_eq!(bus.read8(DMA), 0xC0);
}

#[test]
fn if_unused_bits_read_as_one() {
    let mut bus = Bus::from_rom(vec![0; 0x200]);
    bus.write8(0xFF0F, 0x00);
    assert_eq!(bus.read8(0xFF0F), 0xE0);
    bus.write8(0xFF0F, Interrupt::Serial.mask());
    assert_eq!(bus.read8(0xFF0F), 0xE8);
}

#[test]
fn unmapped_io_reads_ff() {
    let mut bus = Bus::from_rom(vec![0; 0x200]);
    bus.write8(0xFF03, 0x00);
    assert_eq!(bus.read8(0xFF03), 0xFF);
    bus.write8(0xFF4C, 0x55);
    assert_eq!(bus.read8(0xFF4C), 0xFF);
    bus.write8(0xFF68, 0x80);
    assert_eq!(bus.read8(0xFF68), 0xFF);
}

#[test]
fn p1_and_sc_unused_bits_read_as_one() {
    let mut bus = Bus::from_rom(vec![0; 0x200]);
    bus.write8(0xFF00, 0x00);
    assert_eq!(bus.read8(0xFF00) & 0xC0, 0xC0);
    bus.write8(0xFF02, 0x00);
    assert_eq!(bus.read8(0xFF02) & 0x7E, 0x7E);
}

#[test]
fn oam_dma_echo_source_maps_fe_ff_to_wram() {
    use crate::ppu::{DMA, PpuMode};

    let mut bus = Bus::from_rom(vec![0; 0x200]);
    // $FE00 DMA reads WRAM at $DE00; $FF00 → $DF00.
    for i in 0u16..160 {
        bus.write8(0xDE00 + i, 0xA0u8.wrapping_add(i as u8));
        bus.write8(0xDF00 + i, 0xB0u8.wrapping_add(i as u8));
    }

    bus.write8(DMA, 0xFE);
    bus.tick(4 + 4 + 160 * 4);
    bus.ppu.set_line_state(0, 300, PpuMode::HBlank);
    for i in 0u16..160 {
        assert_eq!(
            bus.read8(0xFE00 + i),
            0xA0u8.wrapping_add(i as u8),
            "FE page oam {i}"
        );
    }

    bus.write8(DMA, 0xFF);
    bus.tick(4 + 4 + 160 * 4);
    bus.ppu.set_line_state(0, 300, PpuMode::HBlank);
    for i in 0u16..160 {
        assert_eq!(
            bus.read8(0xFE00 + i),
            0xB0u8.wrapping_add(i as u8),
            "FF page oam {i}"
        );
    }
}

#[test]
fn p1_read_write_through_bus() {
    use crate::input::GameBoyButton;

    let mut bus = Bus::from_rom(vec![0; 0x200]);
    assert_eq!(bus.read8(0xFF00), 0xCF);

    bus.write8(0xFF00, 0x20); // select d-pad
    assert_eq!(bus.read8(0xFF00) & 0x30, 0x20);
    assert_eq!(bus.read8(0xFF00) & 0x0F, 0x0F);

    bus.write8(0xFF0F, 0x00);
    bus.press_button(GameBoyButton::Right);
    assert_eq!(bus.read8(0xFF00) & 0x0F, 0x0E);
    assert_ne!(bus.read8(0xFF0F) & Interrupt::Joypad.mask(), 0);

    bus.write8(0xFF0F, 0x00);
    bus.release_button(GameBoyButton::Right);
    assert_eq!(bus.read8(0xFF00) & 0x0F, 0x0F);
    assert_eq!(bus.read8(0xFF0F) & Interrupt::Joypad.mask(), 0);
}

#[test]
fn p1_select_write_can_request_joypad_if() {
    use crate::input::GameBoyButton;

    let mut bus = Bus::from_rom(vec![0; 0x200]);
    bus.write8(0xFF00, 0x30); // neither group
    bus.write8(0xFF0F, 0x00);
    bus.press_button(GameBoyButton::A); // invisible → no IF
    assert_eq!(bus.read8(0xFF0F) & Interrupt::Joypad.mask(), 0);

    bus.write8(0xFF00, 0x10); // select actions → reveals A
    assert_ne!(bus.read8(0xFF0F) & Interrupt::Joypad.mask(), 0);
    assert_eq!(bus.read8(0xFF00) & 0x0F, 0x0E);
}

#[test]
fn boot_overlay_reads_until_ff50_nonzero() {
    let mut bus = Bus::from_rom(vec![0x42; 0x200]);
    let boot = [0xAAu8; 256];
    bus.enable_boot_rom(&boot);

    assert!(bus.boot_rom_active());
    assert_eq!(bus.read8(0x0000), 0xAA);
    assert_eq!(bus.read8(0x00FF), 0xAA);

    bus.write8(BOOT_DISABLE, 0x00);
    assert!(bus.boot_rom_active());
    assert_eq!(bus.read8(0x0000), 0xAA);

    bus.write8(BOOT_DISABLE, 0x01);
    assert!(!bus.boot_rom_active());
    assert_eq!(bus.read8(0x0000), 0x42);

    bus.write8(BOOT_DISABLE, 0x00);
    assert!(!bus.boot_rom_active());
    assert_eq!(bus.read8(0x0000), 0x42);
}

#[test]
fn disable_boot_rom_forces_off() {
    let mut bus = Bus::from_rom(vec![0x42; 0x200]);
    let boot = [0xBBu8; 256];
    bus.enable_boot_rom(&boot);

    assert!(bus.boot_rom_active());
    assert_eq!(bus.read8(0x0000), 0xBB);

    bus.disable_boot_rom();
    assert!(!bus.boot_rom_active());
    assert_eq!(bus.read8(0x0000), 0x42);
}

#[test]
fn boot_overlay_does_not_block_cart_writes() {
    let mut bus = Bus::from_rom(vec![0x42; 0x200]);
    let boot = [0xAAu8; 256];
    bus.enable_boot_rom(&boot);

    bus.write8(0x2000, 0x01); // MBC1-style noop on rom-only, but write still routes to cart
    assert_eq!(bus.read8(0x0000), 0xAA);
}

fn synthetic_cgb_boot_image() -> [u8; crate::hw::CGB_BOOT_ROM_SIZE] {
    let mut boot = [0x11u8; crate::hw::CGB_BOOT_ROM_SIZE];
    boot[0x000] = 0xA0;
    boot[0x0FF] = 0xA1;
    boot[0x100] = 0xA2; // CPU $0200
    boot[0x7FF] = 0xA3; // CPU $08FF
    boot
}

fn overlay_cart_rom() -> Vec<u8> {
    let mut rom = vec![0x42u8; 0x8000];
    rom[0x0143] = 0xC0;
    rom
}

#[test]
fn cgb_boot_overlay_skips_header_window() {
    use crate::hw::{CgbExecutionMode, HardwareModel};

    let mut bus = Bus::from_rom(overlay_cart_rom()).with_hardware_model(HardwareModel::Cgb {
        mode: CgbExecutionMode::NativeCgb,
    });
    let boot = synthetic_cgb_boot_image();
    bus.enable_cgb_boot_rom(&boot);

    assert!(bus.boot_rom_active());
    assert_eq!(bus.read8(0x0000), 0xA0);
    assert_eq!(bus.read8(0x0143), 0xC0, "$0100–$01FF is cartridge");
    assert_eq!(bus.read8(0x0200), 0xA2);
    assert_eq!(bus.read8(0x08FF), 0xA3);
    assert_eq!(bus.read8(0x0900), 0x42);
}

#[test]
fn cgb_ff50_unmaps_boot_then_cart_is_visible() {
    use crate::hw::{CgbExecutionMode, HardwareModel};

    let mut bus = Bus::from_rom(overlay_cart_rom()).with_hardware_model(HardwareModel::Cgb {
        mode: CgbExecutionMode::NativeCgb,
    });
    bus.enable_cgb_boot_rom(&synthetic_cgb_boot_image());
    bus.write8(BOOT_DISABLE, 0x11);

    assert!(!bus.boot_rom_active());
    assert_eq!(bus.read8(0x0000), 0x42);
    assert_eq!(bus.read8(0x0143), 0xC0);
    assert_eq!(bus.read8(0x0200), 0x42);
    assert_eq!(bus.read8(0x08FF), 0x42);
    assert_eq!(bus.read8(0x0900), 0x42);
}

#[test]
fn cgb_ff50_locks_key0() {
    use crate::hw::{CgbExecutionMode, HardwareModel, KEY0};

    let mut bus = Bus::from_rom(vec![0; 0x200]).with_hardware_model(HardwareModel::Cgb {
        mode: CgbExecutionMode::NativeCgb,
    });
    bus.write8(KEY0, 0x80);
    assert_eq!(bus.read8(KEY0), 0x80);
    bus.write8(BOOT_DISABLE, 0x11);
    bus.write8(KEY0, 0x04);
    assert_eq!(bus.read8(KEY0), 0x80);
}

#[test]
fn dmg_compatibility_routes_key0_and_opri() {
    use crate::hw::{CgbExecutionMode, HardwareModel, KEY0};
    use crate::ppu::OPRI;

    let mut bus = Bus::from_rom(vec![0; 0x200]).with_hardware_model(HardwareModel::Cgb {
        mode: CgbExecutionMode::DmgCompatibility,
    });
    bus.write8(KEY0, 0x04);
    bus.write8(OPRI, 0x01);
    // Pan Docs: KEY0/KEY1/OPRI are CGB Mode only; Non-CGB reads $FF.
    assert_eq!(bus.read8(KEY0), 0xFF);
    assert_eq!(bus.read8(OPRI), 0xFF);
}

#[test]
fn key1_visible_only_in_cgb_mode() {
    use crate::hw::{CgbExecutionMode, HardwareModel, KEY1};

    let mut native = Bus::from_rom(vec![0; 0x200]).with_hardware_model(HardwareModel::Cgb {
        mode: CgbExecutionMode::NativeCgb,
    });
    assert_eq!(native.read8(KEY1) & 0x7E, 0x7E);
    native.write8(KEY1, 0x01);
    assert_eq!(native.read8(KEY1) & 1, 1);

    let mut compat = Bus::from_rom(vec![0; 0x200]).with_hardware_model(HardwareModel::Cgb {
        mode: CgbExecutionMode::DmgCompatibility,
    });
    compat.write8(KEY1, 0x01);
    assert_eq!(compat.read8(KEY1), 0xFF);
}

#[test]
fn dmg_key0_and_opri_stay_unmapped() {
    use crate::hw::KEY0;
    use crate::ppu::OPRI;

    let mut bus = Bus::from_rom(vec![0; 0x200]);
    bus.write8(KEY0, 0x55);
    bus.write8(OPRI, 0x01);
    assert_eq!(bus.read8(KEY0), 0xFF);
    assert_eq!(bus.read8(OPRI), 0xFF);
}

#[test]
fn power_on_keeps_sram_clears_volatile() {
    let mut bus = Bus::new(mbc3_battery_cart());
    bus.write8(0x0000, 0x0A); // enable RAM
    bus.write8(0xA000, 0x5A);
    bus.write8(LCDC, 0x80);
    bus.write8(TIMA, 0xAB);
    bus.write8(0xC000, 0xCD);
    bus.write8(0x2000, 0x02); // ROM bank 2
    assert_eq!(bus.read8(0x4000), 0x22);

    let boot = [0xEEu8; 256];
    bus.enable_boot_rom(&boot);
    assert!(bus.boot_rom_active());

    bus.power_on_keep_battery();

    assert!(!bus.boot_rom_active());
    bus.write8(0x0000, 0x0A);
    assert_eq!(bus.read8(0xA000), 0x5A);
    assert_eq!(bus.read8(LCDC), 0x00);
    assert_eq!(bus.read8(TIMA), 0x00);
    assert_eq!(bus.read8(0xC000), 0x00);
    assert_eq!(bus.read8(0x4000), 0x11);
}

#[test]
fn power_on_keeps_rtc_resets_mapper() {
    let mut bus = Bus::new(mbc3_rtc_cart());
    assert!(bus.cartridge.has_rtc());

    bus.write8(0x2000, 0x02); // ROM bank 2
    assert_eq!(bus.read8(0x4000), 0x22);

    bus.write8(0x0000, 0x0A); // enable RAM/RTC
    bus.write8(0x4000, 0x08); // select RTC seconds
    bus.write8(0xA000, 0x2A);
    bus.write8(0x4000, 0x09); // minutes
    bus.write8(0xA000, 0x17);
    bus.write8(0x6000, 0x00);
    bus.write8(0x6000, 0x01); // latch
    bus.write8(0x4000, 0x08);
    assert_eq!(bus.read8(0xA000), 0x2A);
    bus.write8(0x4000, 0x09);
    assert_eq!(bus.read8(0xA000), 0x17);

    bus.power_on_keep_battery();

    // Mapper power-on: RAM/RTC disabled, ROM bank 1, RAM bank 0 selected.
    assert_eq!(bus.read8(0xA000), 0xFF);
    assert_eq!(bus.read8(0x4000), 0x11);

    bus.write8(0x0000, 0x0A);
    bus.write8(0x4000, 0x00); // RAM bank 0 (not RTC register)
    bus.write8(0xA000, 0xAB);
    assert_eq!(bus.read8(0xA000), 0xAB);

    // RTC clock contents survive; re-select RTC and latch to read.
    bus.write8(0x4000, 0x08);
    bus.write8(0x6000, 0x00);
    bus.write8(0x6000, 0x01);
    assert_eq!(bus.read8(0xA000), 0x2A);
    bus.write8(0x4000, 0x09);
    assert_eq!(bus.read8(0xA000), 0x17);
}

#[test]
fn joypad_interrupt_wakes_halted_cpu() {
    use crate::input::GameBoyButton;

    let mut cpu = Cpu::after_boot();
    cpu.pc = 0x1234;
    cpu.sp = 0xFFFE;
    cpu.halted = true;
    cpu.ime = true;

    let mut bus = Bus::from_rom(vec![0; 0x200]);
    bus.write8(0xFFFF, Interrupt::Joypad.mask());
    bus.write8(0xFF0F, 0x00);
    bus.write8(0xFF00, 0x10); // select actions
    bus.press_button(GameBoyButton::Start);

    assert!(interrupts::poll(&mut cpu, &mut bus));
    assert!(!cpu.halted);
    assert_eq!(cpu.pc, 0x0060);
}

#[test]
fn from_rom_defaults_to_dmg() {
    let bus = Bus::from_rom(vec![0; 0x200]);
    assert_eq!(bus.hardware_model(), crate::hw::HardwareModel::Dmg);
}

#[test]
fn with_hardware_model_copies_native_cgb_bg_attrs() {
    use crate::hw::{CgbExecutionMode, HardwareModel};

    let dmg = Bus::from_rom(vec![0; 0x200]);
    assert!(!dmg.ppu.cgb_bg_attrs());
    assert!(!dmg.ppu.cgb_silicon());

    let mut native = Bus::from_rom(vec![0; 0x200]).with_hardware_model(HardwareModel::Cgb {
        mode: CgbExecutionMode::NativeCgb,
    });
    assert!(native.ppu.cgb_bg_attrs());
    assert!(native.ppu.cgb_silicon());
    native.power_on_keep_battery();
    assert!(native.ppu.cgb_bg_attrs());
    assert!(native.ppu.cgb_silicon());

    let compat = Bus::from_rom(vec![0; 0x200]).with_hardware_model(HardwareModel::Cgb {
        mode: CgbExecutionMode::DmgCompatibility,
    });
    assert!(!compat.ppu.cgb_bg_attrs());
    assert!(compat.ppu.cgb_silicon());
}

/// Gekkio `vblank_stat_intr-C`: CGB silicon (compat included) requests STAT IF
/// one M-cycle before VBlank IF at the LY=144 wrap.
#[test]
fn cgb_compat_mode2_stat_if_one_m_cycle_before_vblank() {
    use crate::hw::{CgbExecutionMode, HardwareModel};
    use crate::ppu::{MODE2_STAT_EARLY, STAT};

    let mut bus = Bus::from_rom(vec![0; 0x200]).with_hardware_model(HardwareModel::Cgb {
        mode: CgbExecutionMode::DmgCompatibility,
    });
    enable_lcd(&mut bus);
    bus.ppu
        .set_line_state(143, MODE2_STAT_EARLY - 1, crate::ppu::PpuMode::HBlank);
    bus.write8(STAT, 0x20);
    bus.write8(0xFF0F, 0x00);

    bus.tick(1);
    assert_eq!(bus.read8(LY), 143);
    assert_ne!(bus.read8(0xFF0F) & Interrupt::LcdStat.mask(), 0);
    assert_eq!(bus.read8(0xFF0F) & Interrupt::VBlank.mask(), 0);

    bus.tick(4);
    assert_eq!(bus.read8(LY), 144);
    assert_ne!(bus.read8(0xFF0F) & Interrupt::VBlank.mask(), 0);
    assert_ne!(bus.read8(0xFF0F) & Interrupt::LcdStat.mask(), 0);
}

/// Default VRAM/WRAM banks match DMG when VBK/SVBK are left untouched.
#[test]
fn cgb_hardware_model_label_does_not_change_memory_map() {
    use crate::hw::{CgbExecutionMode, HardwareModel};
    use crate::ppu::BGP;

    let rom = vec![0u8; 0x200];
    let mut dmg = Bus::from_rom(rom.clone());
    let mut cgb = Bus::from_rom(rom).with_hardware_model(HardwareModel::Cgb {
        mode: CgbExecutionMode::NativeCgb,
    });
    assert_eq!(
        cgb.hardware_model(),
        HardwareModel::Cgb {
            mode: CgbExecutionMode::NativeCgb
        }
    );
    assert_eq!(dmg.hardware_model(), HardwareModel::Dmg);

    let probes = [
        (0x8000_u16, 0x5A_u8),
        (0xC000, 0xA5),
        (0xFF80, 0x3C),
        (BGP, 0xE4),
    ];
    for &(addr, value) in &probes {
        dmg.write8(addr, value);
        cgb.write8(addr, value);
        assert_eq!(
            dmg.read8(addr),
            cgb.read8(addr),
            "read mismatch at ${addr:04X}"
        );
        assert_eq!(dmg.read8(addr), value, "DMG lost write at ${addr:04X}");
        assert_eq!(
            cgb.read8(addr),
            value,
            "CGB label lost write at ${addr:04X}"
        );
    }
}

#[test]
fn dmg_vbk_and_svbk_stay_unmapped_while_vram_wram_work() {
    use super::wram::SVBK;
    use crate::ppu::VBK;

    let mut bus = Bus::from_rom(vec![0; 0x200]);
    // Power-on LCD off: VRAM is CPU-writable without mode pokes.
    assert!(!bus.ppu.lcd_enabled());

    bus.write8(VBK, 0x01);
    bus.write8(SVBK, 0x07);
    assert_eq!(bus.read8(VBK), 0xFF);
    assert_eq!(bus.read8(SVBK), 0xFF);

    bus.write8(0x8000, 0x5A);
    bus.write8(0x9FFF, 0xA5);
    bus.write8(0xC000, 0x11);
    bus.write8(0xD000, 0x22);
    assert_eq!(bus.read8(0x8000), 0x5A);
    assert_eq!(bus.read8(0x9FFF), 0xA5);
    assert_eq!(bus.read8(0xC000), 0x11);
    assert_eq!(bus.read8(0xD000), 0x22);
}

#[test]
fn cgb_vbk_isolates_vram_banks() {
    use crate::hw::{CgbExecutionMode, HardwareModel};
    use crate::ppu::VBK;

    let mut bus = Bus::from_rom(vec![0; 0x200]).with_hardware_model(HardwareModel::Cgb {
        mode: CgbExecutionMode::NativeCgb,
    });
    assert!(!bus.ppu.lcd_enabled());

    bus.write8(VBK, 0x00);
    bus.write8(0x8000, 0x11);
    bus.write8(0x9FFF, 0x22);

    bus.write8(VBK, 0x01);
    assert_eq!(bus.read8(VBK), 0xFF);
    bus.write8(0x8000, 0xAA);
    bus.write8(0x9FFF, 0xBB);
    assert_eq!(bus.read8(0x8000), 0xAA);
    assert_eq!(bus.read8(0x9FFF), 0xBB);

    bus.write8(VBK, 0x00);
    assert_eq!(bus.read8(0x8000), 0x11);
    assert_eq!(bus.read8(0x9FFF), 0x22);
}

#[test]
fn cgb_svbk_zero_aliases_bank_1_and_c000_is_invariant() {
    use super::wram::SVBK;
    use crate::hw::{CgbExecutionMode, HardwareModel};

    let mut bus = Bus::from_rom(vec![0; 0x200]).with_hardware_model(HardwareModel::Cgb {
        mode: CgbExecutionMode::NativeCgb,
    });

    bus.write8(0xC000, 0xC0);
    bus.write8(SVBK, 0x01);
    bus.write8(0xD000, 0xD1);
    bus.write8(SVBK, 0x02);
    bus.write8(0xD000, 0xD2);

    bus.write8(SVBK, 0x00);
    assert_eq!(bus.read8(SVBK), 0xF9);
    assert_eq!(bus.read8(0xD000), 0xD1);
    bus.write8(0xD000, 0xD1 ^ 0x0F);
    bus.write8(SVBK, 0x01);
    assert_eq!(bus.read8(0xD000), 0xD1 ^ 0x0F);

    bus.write8(SVBK, 0x07);
    assert_eq!(bus.read8(0xC000), 0xC0);
    assert_eq!(bus.read8(0xE000), 0xC0);
}

#[test]
fn cgb_power_on_resets_svbk_to_bank_1() {
    use super::wram::SVBK;
    use crate::hw::{CgbExecutionMode, HardwareModel};

    let mut bus = Bus::from_rom(vec![0; 0x200]).with_hardware_model(HardwareModel::Cgb {
        mode: CgbExecutionMode::NativeCgb,
    });
    bus.write8(SVBK, 0x07);
    bus.write8(0xD000, 0x77);
    bus.power_on_keep_battery();
    assert_eq!(bus.read8(SVBK), 0xF9);
    assert_eq!(bus.read8(0xD000), 0x00);
}

#[test]
fn dmg_palette_ports_stay_unmapped_while_vram_wram_work() {
    use crate::ppu::{BGPD, BGPI, OBPD, OBPI};

    let mut bus = Bus::from_rom(vec![0; 0x200]);
    assert!(!bus.ppu.lcd_enabled());

    bus.write8(BGPI, 0x80);
    bus.write8(BGPD, 0x11);
    bus.write8(OBPI, 0x80);
    bus.write8(OBPD, 0x22);
    assert_eq!(bus.read8(BGPI), 0xFF);
    assert_eq!(bus.read8(BGPD), 0xFF);
    assert_eq!(bus.read8(OBPI), 0xFF);
    assert_eq!(bus.read8(OBPD), 0xFF);

    bus.write8(0x8000, 0x5A);
    bus.write8(0xC000, 0xA5);
    bus.write8(0xD000, 0x3C);
    assert_eq!(bus.read8(0x8000), 0x5A);
    assert_eq!(bus.read8(0xC000), 0xA5);
    assert_eq!(bus.read8(0xD000), 0x3C);
}

fn assert_cram_mmio_round_trip_and_auto_inc(bus: &mut Bus) {
    use crate::ppu::{BGPD, BGPI, OBPD, OBPI};

    assert!(
        !bus.ppu.lcd_enabled(),
        "power-on LCDC is off; data ports must be open"
    );

    bus.write8(BGPI, 0x80);
    bus.write8(BGPD, 0x11);
    assert_eq!(bus.read8(BGPI), 0xC1);
    bus.write8(BGPI, 0x80);
    assert_eq!(bus.read8(BGPD), 0x11);

    bus.write8(OBPI, 0x80);
    bus.write8(OBPD, 0x22);
    assert_eq!(bus.read8(OBPI), 0xC1);
    bus.write8(OBPI, 0x80);
    assert_eq!(bus.read8(OBPD), 0x22);
}

#[test]
fn native_cgb_bgpi_bgpd_round_trip_and_auto_inc() {
    use crate::hw::{CgbExecutionMode, HardwareModel};

    let mut bus = Bus::from_rom(vec![0; 0x200]).with_hardware_model(HardwareModel::Cgb {
        mode: CgbExecutionMode::NativeCgb,
    });
    assert_cram_mmio_round_trip_and_auto_inc(&mut bus);
}

#[test]
fn dmg_compatibility_bgpi_mapped_bgpd_unmapped() {
    use crate::hw::{CgbExecutionMode, HardwareModel};
    use crate::ppu::{BGPD, BGPI, OBPD, OBPI};

    let mut bus = Bus::from_rom(vec![0; 0x200]).with_hardware_model(HardwareModel::Cgb {
        mode: CgbExecutionMode::DmgCompatibility,
    });
    bus.write8(BGPI, 0x80);
    bus.write8(BGPD, 0x11);
    bus.write8(OBPI, 0x80);
    bus.write8(OBPD, 0x22);
    // Index ports exist on CGB silicon even in Non-CGB mode (unused bit 6).
    assert_eq!(bus.read8(BGPI), 0xC0);
    assert_eq!(bus.read8(OBPI), 0xC0);
    // Data ports are unused IO (unused_hwio-C test_unmapped $FF69/$FF6B).
    assert_eq!(bus.read8(BGPD), 0xFF);
    assert_eq!(bus.read8(OBPD), 0xFF);
}

#[test]
fn cgb_mode3_bgpd_locked_but_bgpi_still_increments() {
    use crate::hw::{CgbExecutionMode, HardwareModel};
    use crate::ppu::{BGPD, BGPI, LCDC, LCDC_AFTER_BOOT, PpuMode};

    let mut bus = Bus::from_rom(vec![0; 0x200]).with_hardware_model(HardwareModel::Cgb {
        mode: CgbExecutionMode::NativeCgb,
    });
    bus.write8(LCDC, LCDC_AFTER_BOOT);
    bus.ppu.set_line_state(0, 100, PpuMode::PixelTransfer);
    assert!(!bus.ppu.vram_cpu_accessible());

    bus.write8(BGPI, 0x80);
    bus.write8(BGPD, 0x33);
    assert_eq!(bus.read8(BGPI), 0xC1);
    bus.write8(BGPI, 0x80);
    assert_eq!(bus.read8(BGPD), 0xFF);

    bus.write8(LCDC, 0x00);
    bus.write8(BGPI, 0x80);
    assert_eq!(bus.read8(BGPD), 0x00);
}

#[test]
fn cgb_lcd_off_palette_data_ports_are_writable() {
    use crate::hw::{CgbExecutionMode, HardwareModel};
    use crate::ppu::{BGPD, BGPI, OBPD, OBPI};

    let mut bus = Bus::from_rom(vec![0; 0x200]).with_hardware_model(HardwareModel::Cgb {
        mode: CgbExecutionMode::NativeCgb,
    });
    assert!(!bus.ppu.lcd_enabled());

    bus.write8(BGPI, 0x00);
    bus.write8(BGPD, 0xAB);
    assert_eq!(bus.read8(BGPD), 0xAB);
    bus.write8(OBPI, 0x00);
    bus.write8(OBPD, 0xCD);
    assert_eq!(bus.read8(OBPD), 0xCD);
}

fn tick_bus_into_hblank(bus: &mut Bus) {
    use crate::ppu::PpuMode;
    let ly = bus.ppu.ly();
    for _ in 0..CYCLES_PER_LINE {
        if bus.ppu.ly() != ly || bus.ppu.mode() == PpuMode::HBlank {
            break;
        }
        bus.tick(1);
    }
    assert_eq!(bus.ppu.ly(), ly, "expected HBlank on LY={ly}");
    assert_eq!(bus.ppu.mode(), crate::ppu::PpuMode::HBlank);
}

/// LCD off: solid color-3 tile + CRAM pal 0 (color 0 white, color 3 black), then LCDC `$90`.
fn seed_bus_lcdc0_clear_color3_bg(bus: &mut Bus) {
    use crate::ppu::BGP;

    bus.write8(0x8000, 0xFF);
    bus.write8(0x8001, 0xFF);
    // Compat CPU MMIO does not write BGPD; seed CRAM through the PPU port.
    bus.ppu.cram.write_bgpi(0x80);
    bus.ppu.cram.write_bgpd(0xFF, true);
    bus.ppu.cram.write_bgpd(0x7F, true);
    bus.ppu.cram.write_bgpi(0x80 | 6);
    bus.ppu.cram.write_bgpd(0x00, true);
    bus.ppu.cram.write_bgpd(0x00, true);
    bus.write8(BGP, 0b11_10_01_00);
    bus.write8(LCDC, 0b1001_0000);
}

#[test]
fn dmg_compatibility_lcdc0_clear_blanks_bg_unlike_native_cgb() {
    use crate::hw::{CgbExecutionMode, HardwareModel};
    use crate::ppu::Shade;

    let mut compat = Bus::from_rom(vec![0; 0x200]).with_hardware_model(HardwareModel::Cgb {
        mode: CgbExecutionMode::DmgCompatibility,
    });
    assert!(
        !compat.ppu.cgb_bg_attrs(),
        "DmgCompatibility must not enable CGB BG attrs / mix"
    );
    seed_bus_lcdc0_clear_color3_bg(&mut compat);
    tick_bus_into_hblank(&mut compat);
    assert_eq!(
        compat.ppu.framebuffer.pixel(0, 0),
        Shade::Lightest,
        "compat LCDC.0 clear must blank BG like DMG"
    );

    let mut native = Bus::from_rom(vec![0; 0x200]).with_hardware_model(HardwareModel::Cgb {
        mode: CgbExecutionMode::NativeCgb,
    });
    assert!(native.ppu.cgb_bg_attrs());
    seed_bus_lcdc0_clear_color3_bg(&mut native);
    tick_bus_into_hblank(&mut native);
    assert_ne!(
        native.ppu.framebuffer.pixel(0, 0),
        Shade::Lightest,
        "NativeCgb LCDC.0 is master priority, not a BG blank"
    );
}

#[test]
fn dmg_key1_stays_unmapped() {
    use crate::hw::KEY1;

    let mut bus = Bus::from_rom(vec![0; 0x200]);
    assert_eq!(bus.hardware_model(), crate::hw::HardwareModel::Dmg);
    assert_eq!(bus.read8(KEY1), 0xFF);
    bus.write8(KEY1, 0x01);
    assert_eq!(bus.read8(KEY1), 0xFF);
}

#[test]
fn native_cgb_key1_power_on_and_write_mask() {
    use crate::hw::{CgbExecutionMode, HardwareModel, KEY1};

    let mut bus = Bus::from_rom(vec![0; 0x200]).with_hardware_model(HardwareModel::Cgb {
        mode: CgbExecutionMode::NativeCgb,
    });
    assert_eq!(bus.read8(KEY1), 0x7E);
    bus.write8(KEY1, 0x01);
    assert_eq!(bus.read8(KEY1), 0x7F);
    bus.write8(KEY1, 0x80);
    assert_eq!(
        bus.read8(KEY1) & 0x80,
        0,
        "writes must not set current-speed bit 7"
    );
    assert_eq!(bus.read8(KEY1), 0x7E);
}

#[test]
fn dmg_compatibility_hides_key1() {
    use crate::hw::{CgbExecutionMode, HardwareModel, KEY1};

    let mut bus = Bus::from_rom(vec![0; 0x200]).with_hardware_model(HardwareModel::Cgb {
        mode: CgbExecutionMode::DmgCompatibility,
    });
    assert_eq!(bus.read8(KEY1), 0xFF);
    bus.write8(KEY1, 0x01);
    assert_eq!(bus.read8(KEY1), 0xFF);
}

#[test]
fn dmg_compatibility_pcm_cgb_mode_only_not_live() {
    use crate::apu::{NR11, NR12, NR14, NR52, PCM12, PCM34};
    use crate::hw::{CgbExecutionMode, HardwareModel};

    let mut bus = Bus::from_rom(vec![0; 0x200]).with_hardware_model(HardwareModel::Cgb {
        mode: CgbExecutionMode::DmgCompatibility,
    });
    bus.write8(NR52, 0x80);
    bus.write8(NR11, 0x40);
    bus.write8(NR12, 0xF0);
    bus.write8(NR14, 0x80);
    bus.write8(PCM12, 0xFF);
    assert_eq!(
        bus.read8(PCM12),
        0x00,
        "PCM is CGB Mode only: Non-CGB mode stays 0 with CH1 triggered"
    );
    bus.write8(PCM34, 0xFF);
    assert_eq!(bus.read8(PCM34), 0x00);
}

#[test]
fn normal_speed_tick_456_advances_one_scanline() {
    let mut bus = Bus::from_rom(vec![0; 0x200]);
    bus.write8(LCDC, LCDC_AFTER_BOOT);
    assert_eq!(bus.read8(LY), 0);
    bus.tick(456);
    assert_eq!(bus.read8(LY), 1);
}

fn native_cgb_bus() -> Bus {
    use crate::hw::{CgbExecutionMode, HardwareModel};
    Bus::from_rom(vec![0; 0x200]).with_hardware_model(HardwareModel::Cgb {
        mode: CgbExecutionMode::NativeCgb,
    })
}

#[test]
fn cgb_armed_stop_switches_speed_and_clears_armed() {
    use crate::hw::KEY1;

    let mut bus = native_cgb_bus();
    bus.write8(KEY1, 0x01);
    assert!(bus.on_stop());
    assert_eq!(bus.read8(KEY1), 0xFE);
}

#[test]
fn cgb_second_stop_without_rearm_does_not_toggle() {
    use crate::hw::KEY1;

    let mut bus = native_cgb_bus();
    bus.write8(KEY1, 0x01);
    assert!(bus.on_stop());
    assert!(!bus.on_stop());
    assert_eq!(bus.read8(KEY1), 0xFE);
}

#[test]
fn dmg_stop_is_nop_and_key1_stays_unmapped() {
    use crate::hw::KEY1;

    let mut bus = Bus::from_rom(vec![0; 0x200]);
    assert!(!bus.on_stop());
    assert_eq!(bus.read8(KEY1), 0xFF);
}

#[test]
fn speed_switch_resets_and_freezes_div_during_pause() {
    use crate::hw::KEY1;

    let mut bus = native_cgb_bus();
    bus.tick(512);
    assert_ne!(bus.read8(DIV), 0);
    bus.write8(KEY1, 0x01);
    assert!(bus.on_stop());
    assert_eq!(bus.read8(DIV), 0);
}

#[test]
fn after_double_speed_912_cpu_t_is_one_scanline_456_is_not() {
    use crate::hw::KEY1;

    let mut bus = native_cgb_bus();
    bus.write8(KEY1, 0x01);
    assert!(bus.on_stop());
    bus.write8(LCDC, LCDC_AFTER_BOOT);
    assert_eq!(bus.read8(LY), 0);
    bus.tick(456);
    assert_eq!(bus.read8(LY), 0, "456 CPU T is 228 PPU T in Double");
    bus.tick(456);
    assert_eq!(bus.read8(LY), 1, "912 CPU T is 456 PPU T in Double");
}

#[test]
fn double_speed_8192_cpu_t_does_not_step_apu_sequencer() {
    use crate::apu::NR52;
    use crate::hw::KEY1;

    let mut bus = native_cgb_bus();
    bus.write8(NR52, 0x80);
    bus.write8(KEY1, 0x01);
    assert!(bus.on_stop());
    // Pause spent 4100 fixed T; 4092 more reaches the first sequencer step.
    bus.tick(8184);
    assert_eq!(bus.apu.frame_sequencer_step(), 1);
    bus.tick(8192);
    assert_eq!(
        bus.apu.frame_sequencer_step(),
        1,
        "8192 CPU T in Double is 4096 fixed T, not a sequencer period"
    );

    let mut normal = native_cgb_bus();
    normal.write8(NR52, 0x80);
    normal.tick(8192);
    assert_eq!(normal.apu.frame_sequencer_step(), 1);
    normal.tick(8192);
    assert_eq!(
        normal.apu.frame_sequencer_step(),
        2,
        "8192 CPU T in Normal equals one APU sequencer step"
    );
}

fn latch_vram_dma(bus: &mut Bus, src: u16, dest: u16) {
    use crate::hw::{HDMA1, HDMA2, HDMA3, HDMA4};
    bus.write8(HDMA1, (src >> 8) as u8);
    bus.write8(HDMA2, src as u8);
    bus.write8(HDMA3, (dest >> 8) as u8);
    bus.write8(HDMA4, dest as u8);
}

fn vram_bytes(bus: &Bus, addr: u16, n: usize) -> Vec<u8> {
    (0..n)
        .map(|i| bus.ppu.vram.cpu_read(addr + i as u16, true))
        .collect()
}

#[test]
fn dmg_hdma5_reads_ff_and_writes_are_ignored() {
    use crate::hw::HDMA5;

    let mut bus = Bus::from_rom(vec![0; 0x200]);
    assert_eq!(bus.read8(HDMA5), 0xFF);
    bus.write8(HDMA5, 0x00);
    assert_eq!(bus.read8(HDMA5), 0xFF);
    assert_eq!(vram_bytes(&bus, 0x8000, 16), vec![0; 16]);
}

#[test]
fn dmg_compatibility_hdma5_reads_ff_and_writes_are_ignored() {
    use crate::hw::{CgbExecutionMode, HDMA5, HardwareModel, KEY1};

    let mut bus = Bus::from_rom(vec![0; 0x200]).with_hardware_model(HardwareModel::Cgb {
        mode: CgbExecutionMode::DmgCompatibility,
    });
    assert_eq!(bus.read8(HDMA5), 0xFF);
    bus.write8(HDMA5, 0x00);
    assert_eq!(bus.read8(HDMA5), 0xFF);
    assert_eq!(vram_bytes(&bus, 0x8000, 16), vec![0; 16]);
    assert_eq!(bus.read8(KEY1), 0xFF, "KEY1 is CGB Mode only");
}

#[test]
fn native_cgb_gdma_copies_one_rom_block_to_vram() {
    use crate::hw::{HDMA1, HDMA5, KEY1};

    let mut rom = vec![0u8; 0x200];
    for (i, b) in rom.iter_mut().take(16).enumerate() {
        *b = 0xA0 + i as u8;
    }
    let mut bus = Bus::from_rom(rom).with_hardware_model(crate::hw::HardwareModel::Cgb {
        mode: crate::hw::CgbExecutionMode::NativeCgb,
    });
    assert!(!bus.ppu.lcd_enabled(), "LCD off: GDMA still runs");
    latch_vram_dma(&mut bus, 0x0000, 0x8000);
    assert_eq!(bus.read8(HDMA1), 0xFF, "HDMA1–4 are write-only");
    bus.write8(HDMA5, 0x00);
    assert_eq!(
        vram_bytes(&bus, 0x8000, 16),
        (0..16).map(|i| 0xA0 + i).collect::<Vec<_>>()
    );
    assert_eq!(bus.read8(HDMA5), 0xFF);
    assert_eq!(bus.read8(KEY1), 0x7E);
}

#[test]
fn native_cgb_hdma_copies_one_block_on_visible_hblank() {
    use crate::hw::HDMA5;
    use crate::ppu::PpuMode;

    let mut bus = native_cgb_bus();
    for i in 0..16 {
        bus.write8(0xC000 + i, 0xB0 + i as u8);
    }
    enable_lcd(&mut bus);
    assert_ne!(
        bus.ppu.mode(),
        PpuMode::HBlank,
        "start must not be in HBlank"
    );
    latch_vram_dma(&mut bus, 0xC000, 0x9000);
    bus.write8(HDMA5, 0x81); // HBlank, two $10 blocks
    assert_eq!(vram_bytes(&bus, 0x9000, 16), vec![0; 16], "no copy at FF55");
    tick_bus_into_hblank(&mut bus);
    assert_eq!(
        vram_bytes(&bus, 0x9000, 16),
        (0..16).map(|i| 0xB0 + i).collect::<Vec<_>>()
    );
    assert_eq!(
        bus.read8(HDMA5),
        0x00,
        "one block remaining (minus 1 = 0), still active"
    );
}

#[test]
fn native_cgb_hdma_skips_hblank_while_cpu_halted() {
    use crate::hw::HDMA5;

    let mut bus = native_cgb_bus();
    for i in 0..16 {
        bus.write8(0xC000 + i, 0xC0 + i as u8);
    }
    enable_lcd(&mut bus);
    latch_vram_dma(&mut bus, 0xC000, 0x9000);
    bus.write8(HDMA5, 0x80);
    bus.set_cpu_halted(true);
    tick_bus_into_hblank(&mut bus);
    assert_eq!(vram_bytes(&bus, 0x9000, 16), vec![0; 16]);
    assert_eq!(bus.read8(HDMA5) & 0x80, 0, "transfer still active");
}

fn fill_wram_pattern(bus: &mut Bus, addr: u16, n: u16, base: u8) {
    for i in 0..n {
        bus.write8(addr + i, base.wrapping_add(i as u8));
    }
}

fn tick_bus_to_next_hblank(bus: &mut Bus) {
    use crate::ppu::PpuMode;
    let start_ly = bus.ppu.ly();
    while bus.ppu.ly() == start_ly && bus.ppu.mode() == PpuMode::HBlank {
        bus.tick(1);
    }
    tick_bus_into_hblank(bus);
}

/// I: GDMA length `$01` copies two `$10` blocks immediately; HDMA5 reads `$FF`.
#[test]
fn native_cgb_gdma_copies_two_wram_blocks_to_vram() {
    use crate::hw::HDMA5;

    let mut bus = native_cgb_bus();
    assert!(!bus.ppu.lcd_enabled(), "LCD off: GDMA still runs");
    fill_wram_pattern(&mut bus, 0xC000, 32, 0x40);
    latch_vram_dma(&mut bus, 0xC000, 0x8000);
    bus.write8(HDMA5, 0x01);
    assert_eq!(
        vram_bytes(&bus, 0x8000, 32),
        (0..32).map(|i| 0x40 + i).collect::<Vec<_>>()
    );
    assert_eq!(bus.read8(HDMA5), 0xFF);
}

/// GDMA copies even in Mode 3 (Pan Docs: can corrupt the picture).
#[test]
fn native_cgb_gdma_copies_during_mode3() {
    use crate::hw::HDMA5;
    use crate::ppu::PpuMode;

    let mut bus = native_cgb_bus();
    fill_wram_pattern(&mut bus, 0xC000, 16, 0x70);
    enable_lcd(&mut bus);
    bus.ppu.set_line_state(0, 100, PpuMode::PixelTransfer);
    assert!(
        !bus.ppu.vram_cpu_accessible(),
        "CPU VRAM is locked in Mode 3"
    );
    latch_vram_dma(&mut bus, 0xC000, 0x8000);
    bus.write8(HDMA5, 0x00);
    assert_eq!(
        vram_bytes(&bus, 0x8000, 16),
        (0..16).map(|i| 0x70 + i).collect::<Vec<_>>(),
        "GDMA must bypass Mode 3 CPU VRAM lock"
    );
    assert_eq!(bus.read8(HDMA5), 0xFF);
}

/// J: two successive visible HBlanks copy two blocks in order (not both on the first).
#[test]
fn native_cgb_hdma_copies_one_block_per_successive_hblank() {
    use crate::hw::HDMA5;

    let mut bus = native_cgb_bus();
    fill_wram_pattern(&mut bus, 0xC000, 32, 0xD0);
    enable_lcd(&mut bus);
    latch_vram_dma(&mut bus, 0xC000, 0x9000);
    bus.write8(HDMA5, 0x81); // HBlank, two $10 blocks
    tick_bus_into_hblank(&mut bus);
    assert_eq!(
        vram_bytes(&bus, 0x9000, 16),
        (0..16).map(|i| 0xD0 + i).collect::<Vec<_>>(),
        "first HBlank copies block 0"
    );
    assert_eq!(
        vram_bytes(&bus, 0x9010, 16),
        vec![0; 16],
        "first HBlank must not copy block 1"
    );
    assert_eq!(bus.read8(HDMA5), 0x00);

    tick_bus_to_next_hblank(&mut bus);
    assert_eq!(
        vram_bytes(&bus, 0x9010, 16),
        (16..32).map(|i| 0xD0 + i).collect::<Vec<_>>(),
        "second HBlank copies block 1"
    );
    assert_eq!(bus.read8(HDMA5), 0xFF);
}

/// Bulk `Bus::tick` spanning two scanlines must copy one HDMA block per HBlank
/// (not collect both `hblank_lines` and dump them after the PPU has already
/// rendered the second line).
#[test]
fn native_cgb_hdma_bulk_tick_copies_one_block_per_hblank() {
    use crate::hw::HDMA5;
    use crate::ppu::{BGPD, BGPI, Shade};

    let mut bus = native_cgb_bus();
    fill_wram_pattern(&mut bus, 0xC000, 48, 0xE0);
    // Solid color-3 tile in the first HDMA block so line 1 can show the copy.
    for i in 0..16u16 {
        bus.write8(0xC000 + i, 0xFF);
    }
    bus.write8(BGPI, 0x80);
    bus.write8(BGPD, 0xFF);
    bus.write8(BGPD, 0x7F);
    bus.write8(BGPI, 0x80 | 6);
    bus.write8(BGPD, 0x00);
    bus.write8(BGPD, 0x00);

    enable_lcd(&mut bus);
    latch_vram_dma(&mut bus, 0xC000, 0x8000);
    bus.write8(HDMA5, 0x82); // HBlank, three $10 blocks
    assert_eq!(vram_bytes(&bus, 0x8000, 16), vec![0; 16], "no copy at FF55");

    bus.tick(CYCLES_PER_LINE * 2);

    assert_eq!(
        vram_bytes(&bus, 0x8000, 16),
        vec![0xFF; 16],
        "first HBlank copies block 0"
    );
    assert_eq!(
        vram_bytes(&bus, 0x8010, 16),
        (16..32).map(|i| 0xE0 + i).collect::<Vec<_>>(),
        "second HBlank copies block 1"
    );
    assert_eq!(
        vram_bytes(&bus, 0x8020, 16),
        vec![0; 16],
        "third block must wait for a later HBlank"
    );
    assert_eq!(bus.read8(HDMA5), 0x00, "one $10 block still remaining");
    assert_eq!(
        bus.ppu.framebuffer.pixel(0, 1),
        Shade::Darkest,
        "line 1 must render after the first HBlank copy, not from pre-copy VRAM"
    );
}

/// K: abort HDMA with bit7=0 after the first burst; leftover dest unchanged.
#[test]
fn native_cgb_hdma_abort_leaves_remaining_and_skips_next_block() {
    use crate::hw::HDMA5;

    let mut bus = native_cgb_bus();
    fill_wram_pattern(&mut bus, 0xC000, 48, 0x20);
    for i in 0..48u16 {
        bus.write8(0x9000 + i, 0xEE);
    }
    enable_lcd(&mut bus);
    latch_vram_dma(&mut bus, 0xC000, 0x9000);
    bus.write8(HDMA5, 0x82); // HBlank, three $10 blocks
    tick_bus_into_hblank(&mut bus);
    assert_eq!(
        vram_bytes(&bus, 0x9000, 16),
        (0..16).map(|i| 0x20 + i).collect::<Vec<_>>()
    );
    assert_eq!(vram_bytes(&bus, 0x9010, 16), vec![0xEE; 16]);

    bus.write8(HDMA5, 0x00); // bit7=0 while HDMA active → abort, not GDMA
    assert_eq!(
        bus.read8(HDMA5),
        0x81,
        "abort: bit7=1, remaining−1=1 (two $10 blocks left)"
    );
    assert_eq!(
        vram_bytes(&bus, 0x9010, 32),
        vec![0xEE; 32],
        "aborted blocks must not copy"
    );

    tick_bus_to_next_hblank(&mut bus);
    assert_eq!(
        vram_bytes(&bus, 0x9010, 16),
        vec![0xEE; 16],
        "HBlank after abort must not resume the old transfer"
    );
    assert_eq!(bus.read8(HDMA5), 0x81);

    // Restart: latches already point at the second block.
    bus.write8(HDMA5, 0x80);
    tick_bus_to_next_hblank(&mut bus);
    assert_eq!(
        vram_bytes(&bus, 0x9010, 16),
        (16..32).map(|i| 0x20 + i).collect::<Vec<_>>()
    );
    assert_eq!(vram_bytes(&bus, 0x9020, 16), vec![0xEE; 16]);
    assert_eq!(bus.read8(HDMA5), 0xFF);
}

/// L: HDMA/GDMA wall time is 32 PPU T per block; CPU/DIV domain uses 64 T in Double.
#[test]
fn native_cgb_gdma_double_speed_stalls_32_ppu_t_and_64_cpu_t() {
    use crate::hw::{ClockState, HDMA5, KEY1, block_cpu_t, block_fixed_t};
    use crate::ppu::PpuMode;

    assert_eq!(block_cpu_t(ClockState::Normal), 32);
    assert_eq!(block_cpu_t(ClockState::Double), 64);
    assert_eq!(block_fixed_t(), 32);

    let mut normal = native_cgb_bus();
    fill_wram_pattern(&mut normal, 0xC000, 16, 0x55);
    enable_lcd(&mut normal);
    normal.ppu.set_line_state(144, 20, PpuMode::VBlank);
    normal.write8(DIV, 0);
    latch_vram_dma(&mut normal, 0xC000, 0x8000);
    normal.write8(HDMA5, 0x00);
    assert_eq!(vram_bytes(&normal, 0x8000, 16)[0], 0x55);
    assert_eq!(normal.ppu.ly(), 144);
    assert_eq!(normal.ppu.line_cycles(), 20 + 32);
    assert_eq!(normal.read8(DIV), 0, "32 CPU T is below a DIV increment");
    normal.tick(224);
    assert_eq!(normal.read8(DIV), 1, "32 + 224 CPU T = 256 → DIV=1");

    let mut double = native_cgb_bus();
    fill_wram_pattern(&mut double, 0xC000, 16, 0x66);
    double.write8(KEY1, 0x01);
    assert!(double.on_stop());
    enable_lcd(&mut double);
    double.ppu.set_line_state(144, 20, PpuMode::VBlank);
    double.write8(DIV, 0);
    latch_vram_dma(&mut double, 0xC000, 0x8000);
    double.write8(HDMA5, 0x00);
    assert_eq!(vram_bytes(&double, 0x8000, 16)[0], 0x66);
    assert_eq!(double.ppu.ly(), 144);
    assert_eq!(
        double.ppu.line_cycles(),
        20 + 32,
        "GDMA block is 32 PPU T in Double, not 64"
    );
    assert_eq!(
        double.read8(DIV),
        0,
        "64 CPU T is still below a DIV increment"
    );
    double.tick(192);
    assert_eq!(
        double.read8(DIV),
        1,
        "64 CPU T stall + 192 = 256 CPU T → DIV=1 (timer not doubled off)"
    );
}

#[test]
fn native_cgb_ff72_fully_rw() {
    use crate::hw::FF72;

    let mut bus = native_cgb_bus();
    bus.write8(FF72, 0x00);
    assert_eq!(bus.read8(FF72), 0x00);
    bus.write8(FF72, 0xFF);
    assert_eq!(bus.read8(FF72), 0xFF);
}

#[test]
fn native_cgb_ff75_write_zero_reads_8f() {
    use crate::hw::FF75;

    let mut bus = native_cgb_bus();
    bus.write8(FF75, 0x00);
    assert_eq!(bus.read8(FF75), 0x8F);
}

#[test]
fn native_cgb_pcm_writes_ignored_silent_reads_zero() {
    use crate::apu::{PCM12, PCM34};

    let mut bus = native_cgb_bus();
    bus.write8(PCM12, 0xFF);
    assert_eq!(bus.read8(PCM12), 0x00);
    bus.write8(PCM34, 0xFF);
    assert_eq!(bus.read8(PCM34), 0x00);
}

#[test]
fn native_cgb_pcm_follows_generators_until_power_off() {
    use crate::apu::{NR11, NR12, NR14, NR52, PCM12, PCM34};

    let mut bus = native_cgb_bus();
    bus.write8(NR52, 0x80);
    bus.write8(NR11, 0x40);
    bus.write8(NR12, 0xF0);
    bus.write8(NR14, 0x80);
    bus.write8(PCM12, 0xAA);
    assert_eq!(bus.read8(PCM12) & 0x0F, 0x0F, "Native PCM12 tracks CH1");
    bus.write8(NR52, 0x00);
    assert_eq!(bus.read8(PCM12), 0x00);
    assert_eq!(bus.read8(PCM34), 0x00);
}

/// Pan Docs CGB-mode-only vs CGB-silicon MMIO. Compat is not “all CGB ports live.”
#[test]
fn dmg_compatibility_cgb_mmio_visibility_matrix() {
    use super::wram::SVBK;
    use crate::apu::{PCM12, PCM34};
    use crate::hw::{CgbExecutionMode, FF72, FF74, HDMA5, HardwareModel, KEY0, KEY1};
    use crate::ppu::{BGPD, BGPI, OBPD, OBPI, OPRI, VBK};

    let dmg = Bus::from_rom(vec![0; 0x200]);
    let mut compat = Bus::from_rom(vec![0; 0x200]).with_hardware_model(HardwareModel::Cgb {
        mode: CgbExecutionMode::DmgCompatibility,
    });
    let mut native = native_cgb_bus();

    let rows: &[(u16, u8, u8, u8, &str)] = &[
        (KEY0, 0xFF, 0xFF, 0x00, "KEY0 CGB Mode only"),
        (KEY1, 0xFF, 0xFF, 0x7E, "KEY1 CGB Mode only"),
        (VBK, 0xFF, 0xFE, 0xFE, "VBK CGB silicon"),
        (HDMA5, 0xFF, 0xFF, 0xFF, "HDMA CGB Mode only"),
        (BGPI, 0xFF, 0x40, 0x40, "BGPI CGB silicon (bit6 unused)"),
        (BGPD, 0xFF, 0xFF, 0x00, "BGPD CGB Mode only"),
        (OBPI, 0xFF, 0x40, 0x40, "OBPI CGB silicon"),
        (OBPD, 0xFF, 0xFF, 0x00, "OBPD CGB Mode only"),
        (OPRI, 0xFF, 0xFF, 0x00, "OPRI CGB Mode only"),
        (SVBK, 0xFF, 0xFF, 0xF9, "SVBK CGB Mode only"),
        (FF72, 0xFF, 0x00, 0x00, "FF72 CGB silicon"),
        (FF74, 0xFF, 0xFF, 0x00, "FF74 CGB Mode only"),
        (
            PCM12,
            0xFF,
            0x00,
            0x00,
            "PCM12: DMG unmapped, silent 0 on CGB",
        ),
        (PCM34, 0xFF, 0x00, 0x00, "PCM34"),
    ];
    for &(addr, dmg_v, compat_v, native_v, name) in rows {
        assert_eq!(dmg.read8(addr), dmg_v, "DMG {name} ${addr:04X}");
        assert_eq!(compat.read8(addr), compat_v, "compat {name} ${addr:04X}");
        assert_eq!(native.read8(addr), native_v, "native {name} ${addr:04X}");
    }

    compat.write8(BGPD, 0x11);
    assert_eq!(compat.read8(BGPD), 0xFF, "compat BGPD writes ignored");
    native.write8(BGPI, 0x80);
    native.write8(BGPD, 0x11);
    native.write8(BGPI, 0x80);
    assert_eq!(native.read8(BGPD), 0x11, "native BGPD live");
}

#[test]
fn dmg_ff72_stays_unmapped() {
    use crate::hw::FF72;

    let mut bus = Bus::from_rom(vec![0; 0x200]);
    assert_eq!(bus.read8(FF72), 0xFF);
    bus.write8(FF72, 0x00);
    assert_eq!(bus.read8(FF72), 0xFF);
}

#[test]
fn dmg_compatibility_ff72_rw_ff74_locked() {
    use crate::hw::{CgbExecutionMode, FF72, FF74, HardwareModel};

    let mut bus = Bus::from_rom(vec![0; 0x200]).with_hardware_model(HardwareModel::Cgb {
        mode: CgbExecutionMode::DmgCompatibility,
    });
    assert_eq!(bus.read8(FF72), 0x00);
    bus.write8(FF72, 0x5A);
    assert_eq!(bus.read8(FF72), 0x5A);
    assert_eq!(bus.read8(FF74), 0xFF);
    bus.write8(FF74, 0x00);
    assert_eq!(bus.read8(FF74), 0xFF);
}

#[test]
fn native_cgb_ff71_and_ff78_stay_unmapped() {
    let mut bus = native_cgb_bus();
    bus.write8(0xFF71, 0x00);
    assert_eq!(bus.read8(0xFF71), 0xFF);
    bus.write8(0xFF78, 0x00);
    assert_eq!(bus.read8(0xFF78), 0xFF);
    bus.write8(0xFF7F, 0x00);
    assert_eq!(bus.read8(0xFF7F), 0xFF);
}
