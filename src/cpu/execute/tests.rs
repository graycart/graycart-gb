use super::*;
use crate::cpu::Reg16;

fn boot_machine(rom: Vec<u8>) -> (Cpu, Bus) {
    let mut bus = Bus::from_rom(rom);
    bus.write8(crate::ppu::LCDC, crate::ppu::LCDC_AFTER_BOOT);
    (Cpu::after_boot(), bus)
}

fn boot_cgb_machine(rom: Vec<u8>) -> (Cpu, Bus) {
    use crate::hw::{CgbExecutionMode, HardwareModel};
    let mut bus = Bus::from_rom(rom).with_hardware_model(HardwareModel::Cgb {
        mode: CgbExecutionMode::NativeCgb,
    });
    bus.write8(crate::ppu::LCDC, crate::ppu::LCDC_AFTER_BOOT);
    (Cpu::after_boot(), bus)
}

#[test]
fn entry_point_nop_then_jp_reaches_0150() {
    // Minimal ROM image large enough for $0150.
    let mut rom = vec![0x00; 0x200];
    rom[0x100] = 0x00; // NOP
    rom[0x101] = 0xC3; // JP $0150
    rom[0x102] = 0x50;
    rom[0x103] = 0x01;
    rom[0x150] = 0xFE; // CP $11 (not executed yet)

    let (mut cpu, mut bus) = boot_machine(rom);

    let d0 = step(&mut cpu, &mut bus).unwrap();
    assert_eq!(d0.instruction, Instruction::Nop);
    assert_eq!(cpu.pc, 0x0101);

    let d1 = step(&mut cpu, &mut bus).unwrap();
    assert_eq!(
        d1.instruction,
        Instruction::Jp {
            cond: None,
            target: 0x0150
        }
    );
    assert_eq!(cpu.pc, 0x0150);
}

#[test]
fn push_pop_af_clears_flag_low_nibble() {
    let mut rom = vec![0x00; 0x200];
    rom[0x100] = 0xF5; // PUSH AF
    rom[0x101] = 0xF1; // POP AF

    let (mut cpu, mut bus) = boot_machine(rom);
    cpu.set_af(0x12AB); // low nibble of F should not survive
    assert_eq!(cpu.f(), 0xA0);

    step(&mut cpu, &mut bus).unwrap();
    assert_eq!(cpu.sp, 0xFFFC);

    cpu.a = 0x00;
    cpu.set_f(0x00);
    step(&mut cpu, &mut bus).unwrap();
    assert_eq!(cpu.af(), 0x12A0);
    assert_eq!(cpu.sp, 0xFFFE);
}

#[test]
fn call_pushes_return_address_then_ret() {
    let mut rom = vec![0x00; 0x200];
    rom[0x100] = 0xCD; // CALL $0180
    rom[0x101] = 0x80;
    rom[0x102] = 0x01;
    rom[0x103] = 0x00; // return lands here
    rom[0x180] = 0xC9; // RET

    let (mut cpu, mut bus) = boot_machine(rom);
    step(&mut cpu, &mut bus).unwrap();
    assert_eq!(cpu.pc, 0x0180);
    assert_eq!(bus.read16(cpu.sp), 0x0103);

    step(&mut cpu, &mut bus).unwrap();
    assert_eq!(cpu.pc, 0x0103);
    assert_eq!(cpu.sp, 0xFFFE);
}

#[test]
fn rst_pushes_return_and_jumps_to_vector() {
    let mut rom = vec![0x00; 0x200];
    rom[0x100] = 0xEF; // RST $28
    rom[0x101] = 0x00; // return lands here
    rom[0x28] = 0xC9; // RET

    let (mut cpu, mut bus) = boot_machine(rom);
    step(&mut cpu, &mut bus).unwrap();
    assert_eq!(cpu.pc, 0x0028);
    assert_eq!(bus.read16(cpu.sp), 0x0101);

    step(&mut cpu, &mut bus).unwrap();
    assert_eq!(cpu.pc, 0x0101);
    assert_eq!(cpu.sp, 0xFFFE);
}

#[test]
fn reti_pops_pc_enables_ime_immediately_and_clears_delayed_ei() {
    use super::super::stack::push16;

    let mut rom = vec![0x00; 0x200];
    rom[0x180] = 0xD9; // RETI
    rom[0x103] = 0x00; // return target

    let (mut cpu, mut bus) = boot_machine(rom);
    cpu.pc = 0x0180;
    cpu.sp = 0xFFFE;
    cpu.ime = false;
    cpu.ime_enable_pending = true;
    cpu.ime_enable_armed = true;
    push16(&mut cpu, &mut bus, 0x0103);
    assert_eq!(cpu.sp, 0xFFFC);
    assert_eq!(bus.read8(0xFFFC), 0x03);
    assert_eq!(bus.read8(0xFFFD), 0x01);

    step(&mut cpu, &mut bus).unwrap();
    assert_eq!(cpu.pc, 0x0103);
    assert_eq!(cpu.sp, 0xFFFE);
    assert!(cpu.ime);
    assert!(!cpu.ime_enable_pending);
    assert!(!cpu.ime_enable_armed);
}

#[test]
fn call_z_taken_and_not_taken() {
    let mut rom = vec![0x00; 0x200];
    rom[0x100] = 0xCC; // CALL Z,$0180
    rom[0x101] = 0x80;
    rom[0x102] = 0x01;
    rom[0x103] = 0x00;
    rom[0x180] = 0xC9; // RET

    // Z clear → not taken
    let (mut cpu, mut bus) = boot_machine(rom.clone());
    cpu.set_flag_z(false);
    let sp0 = cpu.sp;
    step(&mut cpu, &mut bus).unwrap();
    assert_eq!(cpu.pc, 0x0103);
    assert_eq!(cpu.sp, sp0);

    // Z set → taken, then RET restores
    let (mut cpu, mut bus) = boot_machine(rom);
    cpu.set_flag_z(true);
    step(&mut cpu, &mut bus).unwrap();
    assert_eq!(cpu.pc, 0x0180);
    assert_eq!(bus.read16(cpu.sp), 0x0103);
    step(&mut cpu, &mut bus).unwrap();
    assert_eq!(cpu.pc, 0x0103);
    assert_eq!(cpu.sp, 0xFFFE);
}

#[test]
fn call_nz_nc_c_respect_conditions() {
    // C4 CALL NZ: taken when Z clear
    let mut rom = vec![0x00; 0x200];
    rom[0x100] = 0xC4;
    rom[0x101] = 0x80;
    rom[0x102] = 0x01;
    let (mut cpu, mut bus) = boot_machine(rom.clone());
    cpu.set_flag_z(false);
    step(&mut cpu, &mut bus).unwrap();
    assert_eq!(cpu.pc, 0x0180);
    let (mut cpu, mut bus) = boot_machine(rom);
    cpu.set_flag_z(true);
    step(&mut cpu, &mut bus).unwrap();
    assert_eq!(cpu.pc, 0x0103);

    // D4 CALL NC: taken when C clear
    let mut rom = vec![0x00; 0x200];
    rom[0x100] = 0xD4;
    rom[0x101] = 0x80;
    rom[0x102] = 0x01;
    let (mut cpu, mut bus) = boot_machine(rom.clone());
    cpu.set_flag_c(false);
    step(&mut cpu, &mut bus).unwrap();
    assert_eq!(cpu.pc, 0x0180);
    let (mut cpu, mut bus) = boot_machine(rom);
    cpu.set_flag_c(true);
    step(&mut cpu, &mut bus).unwrap();
    assert_eq!(cpu.pc, 0x0103);

    // DC CALL C: taken when C set
    let mut rom = vec![0x00; 0x200];
    rom[0x100] = 0xDC;
    rom[0x101] = 0x80;
    rom[0x102] = 0x01;
    let (mut cpu, mut bus) = boot_machine(rom.clone());
    cpu.set_flag_c(true);
    step(&mut cpu, &mut bus).unwrap();
    assert_eq!(cpu.pc, 0x0180);
    let (mut cpu, mut bus) = boot_machine(rom);
    cpu.set_flag_c(false);
    step(&mut cpu, &mut bus).unwrap();
    assert_eq!(cpu.pc, 0x0103);
}

#[test]
fn ld16_and_ld8() {
    let mut rom = vec![0x00; 0x200];
    rom[0x100] = 0x21; // LD HL, $ABCD
    rom[0x101] = 0xCD;
    rom[0x102] = 0xAB;
    rom[0x103] = 0x3E; // LD A, $42
    rom[0x104] = 0x42;
    rom[0x105] = 0x47; // LD B, A

    let (mut cpu, mut bus) = boot_machine(rom);
    step(&mut cpu, &mut bus).unwrap();
    assert_eq!(cpu.hl(), 0xABCD);
    step(&mut cpu, &mut bus).unwrap();
    assert_eq!(cpu.a, 0x42);
    step(&mut cpu, &mut bus).unwrap();
    assert_eq!(cpu.b, 0x42);
    assert_eq!(cpu.read_r16(Reg16::Hl), 0xABCD);
}

#[test]
fn ld_hl_sp_e_sets_hl_flags_leaves_sp() {
    let mut rom = vec![0x00; 0x200];
    rom[0x100] = 0xF8; // LD HL, SP+1
    rom[0x101] = 0x01;

    let (mut cpu, mut bus) = boot_machine(rom);
    cpu.sp = 0x000F;
    cpu.set_flag_z(true);
    cpu.set_flag_n(true);
    cpu.set_flag_h(false);
    cpu.set_flag_c(true);

    step(&mut cpu, &mut bus).unwrap();
    assert_eq!(cpu.hl(), 0x0010);
    assert_eq!(cpu.sp, 0x000F);
    assert!(!cpu.flag_z());
    assert!(!cpu.flag_n());
    assert!(cpu.flag_h());
    assert!(!cpu.flag_c());
    assert_eq!(cpu.pc, 0x0102);
}

#[test]
fn ld_hl_sp_e_negative_offset() {
    let mut rom = vec![0x00; 0x200];
    rom[0x100] = 0xF8; // LD HL, SP-1
    rom[0x101] = 0xFF;

    let (mut cpu, mut bus) = boot_machine(rom);
    cpu.sp = 0x1000;
    step(&mut cpu, &mut bus).unwrap();
    assert_eq!(cpu.hl(), 0x0FFF);
    assert_eq!(cpu.sp, 0x1000);
    assert!(!cpu.flag_z());
    assert!(!cpu.flag_n());
    assert!(!cpu.flag_h());
    assert!(!cpu.flag_c());
}

#[test]
fn ld_sp_hl_copies_and_preserves_flags() {
    let mut rom = vec![0x00; 0x200];
    rom[0x100] = 0xF9; // LD SP, HL

    let (mut cpu, mut bus) = boot_machine(rom);
    cpu.write_r16(Reg16::Hl, 0xC3A0);
    cpu.sp = 0xDFDD;
    cpu.set_flag_z(true);
    cpu.set_flag_n(true);
    cpu.set_flag_h(true);
    cpu.set_flag_c(true);
    let f = cpu.f();

    step(&mut cpu, &mut bus).unwrap();
    assert_eq!(cpu.sp, 0xC3A0);
    assert_eq!(cpu.hl(), 0xC3A0);
    assert_eq!(cpu.f(), f);
    assert_eq!(cpu.pc, 0x0101);
}

#[test]
fn add_sp_e_updates_sp_flags_leaves_hl() {
    let mut rom = vec![0x00; 0x200];
    rom[0x100] = 0xE8; // ADD SP, +1
    rom[0x101] = 0x01;

    let (mut cpu, mut bus) = boot_machine(rom);
    cpu.sp = 0x000F;
    cpu.write_r16(Reg16::Hl, 0xABCD);
    cpu.set_flag_z(true);
    cpu.set_flag_n(true);

    let expected = super::super::alu::add_sp_e8(0x000F, 1);
    step(&mut cpu, &mut bus).unwrap();
    assert_eq!(cpu.sp, expected.value);
    assert_eq!(cpu.hl(), 0xABCD);
    assert!(!cpu.flag_z());
    assert!(!cpu.flag_n());
    assert_eq!(cpu.flag_h(), expected.h);
    assert_eq!(cpu.flag_c(), expected.c);
    assert_eq!(cpu.pc, 0x0102);
}

#[test]
fn add_sp_e_negative_offset() {
    let mut rom = vec![0x00; 0x200];
    rom[0x100] = 0xE8; // ADD SP, -1
    rom[0x101] = 0xFF;

    let (mut cpu, mut bus) = boot_machine(rom);
    cpu.sp = 0x1000;
    let expected = super::super::alu::add_sp_e8(0x1000, -1);
    step(&mut cpu, &mut bus).unwrap();
    assert_eq!(cpu.sp, expected.value);
    assert_eq!(cpu.flag_h(), expected.h);
    assert_eq!(cpu.flag_c(), expected.c);
    assert!(!cpu.flag_z());
    assert!(!cpu.flag_n());
}

#[test]
fn cp_sets_flags_without_changing_a() {
    let mut rom = vec![0x00; 0x200];
    rom[0x100] = 0xFE; // CP $11
    rom[0x101] = 0x11;

    let (mut cpu, mut bus) = boot_machine(rom);
    assert_eq!(cpu.a, 0x01);

    step(&mut cpu, &mut bus).unwrap();
    assert_eq!(cpu.a, 0x01);
    assert!(!cpu.flag_z());
    assert!(cpu.flag_n());
    assert!(!cpu.flag_h()); // 0x1 - 0x1, no low-nibble borrow
    assert!(cpu.flag_c());
}

#[test]
fn pokemon_path_cp_jr_xor_jr() {
    // Mirror Red's $0150 sequence after jumping there.
    let mut rom = vec![0x00; 0x200];
    rom[0x100] = 0x00; // NOP
    rom[0x101] = 0xC3; // JP $0150
    rom[0x102] = 0x50;
    rom[0x103] = 0x01;
    rom[0x150] = 0xFE; // CP $11
    rom[0x151] = 0x11;
    rom[0x152] = 0x28; // JR Z, $0157 (not taken)
    rom[0x153] = 0x03;
    rom[0x154] = 0xAF; // XOR A
    rom[0x155] = 0x18; // JR $0159
    rom[0x156] = 0x02;
    rom[0x157] = 0x3E; // would be LD A,$00 if Z path taken
    rom[0x158] = 0x00;
    rom[0x159] = 0x00; // landing NOP

    let (mut cpu, mut bus) = boot_machine(rom);

    step(&mut cpu, &mut bus).unwrap(); // NOP
    step(&mut cpu, &mut bus).unwrap(); // JP
    assert_eq!(cpu.pc, 0x0150);

    step(&mut cpu, &mut bus).unwrap(); // CP
    assert_eq!(cpu.pc, 0x0152);
    assert!(!cpu.flag_z());

    step(&mut cpu, &mut bus).unwrap(); // JR Z not taken
    assert_eq!(cpu.pc, 0x0154);

    step(&mut cpu, &mut bus).unwrap(); // XOR A
    assert_eq!(cpu.a, 0x00);
    assert!(cpu.flag_z());
    assert!(!cpu.flag_n());
    assert!(!cpu.flag_h());
    assert!(!cpu.flag_c());
    assert_eq!(cpu.pc, 0x0155);

    step(&mut cpu, &mut bus).unwrap(); // JR $0159
    assert_eq!(cpu.pc, 0x0159);
}

#[test]
fn ldi_mem_hl_a_writes_and_increments() {
    let mut rom = vec![0x00; 0x200];
    rom[0x100] = 0x21; // LD HL,$C000
    rom[0x101] = 0x00;
    rom[0x102] = 0xC0;
    rom[0x103] = 0x3E; // LD A,$5A
    rom[0x104] = 0x5A;
    rom[0x105] = 0x22; // LD (HL+), A

    let (mut cpu, mut bus) = boot_machine(rom);
    step(&mut cpu, &mut bus).unwrap();
    step(&mut cpu, &mut bus).unwrap();
    step(&mut cpu, &mut bus).unwrap();
    assert_eq!(bus.read8(0xC000), 0x5A);
    assert_eq!(cpu.hl(), 0xC001);
}

#[test]
fn ei_is_delayed_one_instruction() {
    let mut rom = vec![0x00; 0x200];
    rom[0x100] = 0xFB; // EI
    rom[0x101] = 0x00; // NOP
    rom[0x102] = 0x00; // NOP

    let (mut cpu, mut bus) = boot_machine(rom);
    assert!(!cpu.ime);

    step(&mut cpu, &mut bus).unwrap(); // EI
    assert!(!cpu.ime);
    assert!(cpu.ime_enable_armed); // pending promoted to armed at end of EI
    assert!(!cpu.ime_enable_pending);

    step(&mut cpu, &mut bus).unwrap(); // NOP — IME enables after this
    assert!(cpu.ime);
    assert!(!cpu.ime_enable_armed);
    assert!(!cpu.ime_enable_pending);
}

#[test]
fn ei_then_di_leaves_interrupts_disabled() {
    let mut rom = vec![0x00; 0x200];
    rom[0x100] = 0xFB; // EI
    rom[0x101] = 0xF3; // DI

    let (mut cpu, mut bus) = boot_machine(rom);
    step(&mut cpu, &mut bus).unwrap();
    step(&mut cpu, &mut bus).unwrap();
    assert!(!cpu.ime);
    assert!(!cpu.ime_enable_pending);
}

#[test]
fn interrupt_from_halt_costs_extra_m_cycle() {
    // TCAGBD §4.9: 5 M-cycles normally, +1 M-cycle when leaving HALT.
    let mut rom = vec![0x00; 0x200];
    rom[0x0050] = 0x00; // timer vector

    let (mut cpu, mut bus) = boot_machine(rom);
    cpu.pc = 0x0100;
    cpu.sp = 0xFFFE;
    cpu.ime = true;
    cpu.halted = true;
    bus.write8(0xFFFF, 0x04); // IE timer
    bus.write8(0xFF0F, 0x04); // IF timer

    let ly_before = bus.ppu.ly();
    let line_before = bus.ppu.line_cycles();
    step(&mut cpu, &mut bus).unwrap();
    assert_eq!(cpu.pc, 0x0050);
    let advanced = if bus.ppu.ly() == ly_before {
        bus.ppu.line_cycles() - line_before
    } else {
        (456 - line_before) + bus.ppu.line_cycles()
    };
    assert_eq!(advanced, 24);
}

#[test]
fn ld_r16_imm_is_three_m_cycles() {
    // Pan Docs opcode table: `ld rr,nn` is 3 M-cycles (12 T), not 4.
    let mut rom = vec![0x00; 0x200];
    rom[0x100] = 0x21; // ld hl, $FF80
    rom[0x101] = 0x80;
    rom[0x102] = 0xFF;

    let (mut cpu, mut bus) = boot_machine(rom);
    let line_before = bus.ppu.line_cycles();
    step(&mut cpu, &mut bus).unwrap();
    assert_eq!(cpu.hl(), 0xFF80);
    assert_eq!(bus.ppu.line_cycles() - line_before, 12);
}

fn line_delta(bus: &crate::bus::Bus, before_ly: u8, before_dot: u32) -> u32 {
    if bus.ppu.ly() == before_ly {
        bus.ppu.line_cycles() - before_dot
    } else {
        (456 - before_dot) + bus.ppu.line_cycles()
    }
}

#[test]
fn opcode_family_t_cycles_match_pandocs() {
    // Pan Docs / pastraiser family timings (T-cycles). Conditional extras in `step`.
    let cases: &[(&[u8], u32)] = &[
        (&[0x00], 4),              // NOP
        (&[0x21, 0x00, 0x00], 12), // LD HL,nn
        (&[0x36, 0x00], 12),       // LD (HL),n
        (&[0x3E, 0x00], 8),        // LD A,n
        (&[0x34], 12),             // INC (HL)
        (&[0x86], 8),              // ADD (HL)
        (&[0xCB, 0x06], 16),       // RLC (HL)
        (&[0x08, 0x00, 0xC0], 20), // LD (nn),SP
        (&[0xC3, 0x50, 0x01], 16), // JP nn
        (&[0xC2, 0x50, 0x01], 12), // JP NZ,nn (Z=1 after boot → not taken)
        (&[0xCD, 0x50, 0x01], 24), // CALL nn
        (&[0xC4, 0x50, 0x01], 12), // CALL NZ (not taken)
        (&[0xC9], 16),             // RET
        (&[0xC0], 8),              // RET NZ (not taken)
        (&[0x18, 0x00], 12),       // JR 0 taken
        (&[0x20, 0x00], 8),        // JR NZ (not taken)
        (&[0xCB, 0x46], 12),       // BIT 0,(HL)
        (&[0xCB, 0x40], 8),        // BIT 0,B
        (&[0xE0, 0x01], 12),       // LDH (n),A
        (&[0xE2], 12),             // LD (C),A
        (&[0x03], 8),              // INC BC
        (&[0x33], 8),              // INC SP
        (&[0xF9], 8),              // LD SP,HL
        (&[0xE8, 0x01], 16),       // ADD SP,e
        (&[0xF8, 0x01], 12),       // LD HL,SP+e
        (&[0xC7], 16),             // RST 00
        (&[0xF5], 16),             // PUSH AF
        (&[0xF1], 12),             // POP AF
        (&[0xEA, 0x00, 0xC0], 16), // LD (nn),A
        (&[0xFA, 0x00, 0xC0], 16), // LD A,(nn)
    ];
    for (bytes, expect) in cases {
        let mut rom = vec![0x00; 0x200];
        rom[0x100..0x100 + bytes.len()].copy_from_slice(bytes);
        let (mut cpu, mut bus) = boot_machine(rom);
        cpu.sp = 0xFFFE;
        let ly = bus.ppu.ly();
        let dot = bus.ppu.line_cycles();
        step(&mut cpu, &mut bus).unwrap();
        assert_eq!(
            line_delta(&bus, ly, dot),
            *expect,
            "opcode {bytes:02X?} T-cycles"
        );
    }
}

#[test]
fn call_cc_taken_is_six_m_cycles() {
    let mut rom = vec![0x00; 0x200];
    rom[0x100] = 0xCC; // CALL Z
    rom[0x101] = 0x50;
    rom[0x102] = 0x01;
    let (mut cpu, mut bus) = boot_machine(rom);
    cpu.sp = 0xFFFE;
    assert!(cpu.f() & 0x80 != 0, "after-boot Z=1");
    let ly = bus.ppu.ly();
    let dot = bus.ppu.line_cycles();
    step(&mut cpu, &mut bus).unwrap();
    assert_eq!(cpu.pc, 0x0150);
    assert_eq!(line_delta(&bus, ly, dot), 24);
}

#[test]
fn ret_cc_taken_is_five_m_cycles() {
    let mut rom = vec![0x00; 0x200];
    rom[0x100] = 0xC8; // RET Z
    rom[0x150] = 0x00;
    let (mut cpu, mut bus) = boot_machine(rom);
    cpu.sp = 0xFFFE;
    bus.write8(0xFFFD, 0x50);
    bus.write8(0xFFFE, 0x01);
    cpu.sp = 0xFFFD;
    let ly = bus.ppu.ly();
    let dot = bus.ppu.line_cycles();
    step(&mut cpu, &mut bus).unwrap();
    assert_eq!(cpu.pc, 0x0150);
    assert_eq!(line_delta(&bus, ly, dot), 20);
}

#[test]
fn ld_a_hl_samples_stat_after_fetch_m_cycle() {
    // `ld a,(hl)` is 2 M-cycles; STAT must be read on the second, after 4 T.
    let mut rom = vec![0x00; 0x200];
    rom[0x100] = 0x21; // ld hl, $FF41
    rom[0x101] = 0x41;
    rom[0x102] = 0xFF;
    rom[0x103] = 0x7E; // ld a,(hl)

    let (mut cpu, mut bus) = boot_machine(rom);
    step(&mut cpu, &mut bus).unwrap(); // ld hl
    assert_eq!(cpu.hl(), 0xFF41);
    bus.ppu.set_line_state(0, 76, crate::ppu::PpuMode::OamScan);
    step(&mut cpu, &mut bus).unwrap(); // ld a,(hl): 4 T → mode 3, then read
    assert_eq!(bus.ppu.mode(), crate::ppu::PpuMode::PixelTransfer);
    assert_eq!(cpu.a & 0x03, 3);
    assert_eq!(bus.ppu.line_cycles(), 84);
}

#[test]
fn halt_stops_fetching_until_manually_cleared() {
    let mut rom = vec![0x00; 0x200];
    rom[0x100] = 0x76; // HALT
    rom[0x101] = 0x00; // would be NOP if we kept fetching

    let (mut cpu, mut bus) = boot_machine(rom);
    let d = step(&mut cpu, &mut bus).unwrap();
    assert_eq!(d.instruction, Instruction::Halt);
    assert!(cpu.halted);
    assert_eq!(cpu.pc, 0x0101);

    let d2 = step(&mut cpu, &mut bus).unwrap();
    assert_eq!(d2.instruction, Instruction::Halt);
    assert_eq!(d2.len, 0);
    assert!(cpu.halted);
    assert_eq!(cpu.pc, 0x0101); // did not advance into the NOP

    cpu.halted = false;
    let d3 = step(&mut cpu, &mut bus).unwrap();
    assert_eq!(d3.instruction, Instruction::Nop);
    assert_eq!(cpu.pc, 0x0102);
}

/// Pan Docs: HDMA also halts on CPU HALT. HALT sets [`Bus::set_cpu_halted`]
/// so HBlank bursts skip until the CPU resumes.
#[test]
fn halt_instruction_sets_bus_cpu_halted() {
    let mut rom = vec![0x00; 0x200];
    rom[0x100] = 0x76; // HALT

    let (mut cpu, mut bus) = boot_cgb_machine(rom);
    assert!(!bus.cpu_halted());
    let d = step(&mut cpu, &mut bus).unwrap();
    assert_eq!(d.instruction, Instruction::Halt);
    assert!(cpu.halted);
    assert!(
        bus.cpu_halted(),
        "HALT must set_cpu_halted(true) before remaining ticks"
    );
}

#[test]
fn halted_step_syncs_bus_cpu_halted_before_tick() {
    let rom = vec![0x00; 0x200];
    let (mut cpu, mut bus) = boot_cgb_machine(rom);
    cpu.halted = true;
    assert!(!bus.cpu_halted());
    let d = step(&mut cpu, &mut bus).unwrap();
    assert_eq!(d.instruction, Instruction::Halt);
    assert!(cpu.halted);
    assert!(
        bus.cpu_halted(),
        "halted step must set_cpu_halted(true) before the 4 T idle tick"
    );
}

#[test]
fn halt_wake_clears_bus_cpu_halted_before_dispatch_tick() {
    let mut rom = vec![0x00; 0x200];
    rom[0x0050] = 0x00; // timer vector

    let (mut cpu, mut bus) = boot_cgb_machine(rom);
    cpu.pc = 0x0100;
    cpu.sp = 0xFFFE;
    cpu.ime = true;
    cpu.halted = true;
    bus.set_cpu_halted(true);
    bus.write8(0xFFFF, 0x04); // IE timer
    bus.write8(0xFF0F, 0x04); // IF timer

    step(&mut cpu, &mut bus).unwrap();
    assert!(!cpu.halted);
    assert!(
        !bus.cpu_halted(),
        "wake/poll must set_cpu_halted(false) before the interrupt tick"
    );
}

/// Native CGB: executing HALT must freeze HDMA so a later HBlank copies nothing.
#[test]
fn native_cgb_halt_pauses_hdma_hblank_copy() {
    use crate::hw::{HDMA1, HDMA2, HDMA3, HDMA4, HDMA5};
    use crate::ppu::{CYCLES_PER_LINE, PpuMode};

    let mut rom = vec![0x00; 0x200];
    rom[0x100] = 0x76; // HALT
    let (mut cpu, mut bus) = boot_cgb_machine(rom);
    for i in 0..16u16 {
        bus.write8(0xC000 + i, 0xC0 + i as u8);
    }
    assert_ne!(
        bus.ppu.mode(),
        PpuMode::HBlank,
        "HDMA start must not be in HBlank"
    );
    bus.write8(HDMA1, 0xC0);
    bus.write8(HDMA2, 0x00);
    bus.write8(HDMA3, 0x90);
    bus.write8(HDMA4, 0x00);
    bus.write8(HDMA5, 0x80); // HBlank, one $10 block

    step(&mut cpu, &mut bus).unwrap();
    assert!(cpu.halted);
    assert!(bus.cpu_halted());

    let ly = bus.ppu.ly();
    for _ in 0..CYCLES_PER_LINE {
        if bus.ppu.ly() != ly || bus.ppu.mode() == PpuMode::HBlank {
            break;
        }
        step(&mut cpu, &mut bus).unwrap();
    }
    assert_eq!(bus.ppu.mode(), PpuMode::HBlank);
    let dest: Vec<u8> = (0..16)
        .map(|i| bus.ppu.vram.cpu_read(0x9000 + i, true))
        .collect();
    assert_eq!(dest, vec![0; 16], "HBlank during HALT must not copy");
}

#[test]
fn scf_ccf_preserve_z() {
    let mut rom = vec![0x00; 0x200];
    rom[0x100] = 0xAF; // XOR A → Z=1, C=0
    rom[0x101] = 0x37; // SCF
    rom[0x102] = 0x3F; // CCF
    rom[0x103] = 0x3F; // CCF again

    let (mut cpu, mut bus) = boot_machine(rom);
    step(&mut cpu, &mut bus).unwrap();
    assert!(cpu.flag_z());
    assert!(!cpu.flag_c());

    step(&mut cpu, &mut bus).unwrap(); // SCF
    assert!(cpu.flag_z());
    assert!(!cpu.flag_n());
    assert!(!cpu.flag_h());
    assert!(cpu.flag_c());

    step(&mut cpu, &mut bus).unwrap(); // CCF → 0
    assert!(cpu.flag_z());
    assert!(!cpu.flag_c());
    assert!(!cpu.flag_n());
    assert!(!cpu.flag_h());

    step(&mut cpu, &mut bus).unwrap(); // CCF → 1
    assert!(cpu.flag_z());
    assert!(cpu.flag_c());
}

#[test]
fn rr_d_carry_in_out_and_zero() {
    let mut rom = vec![0x00; 0x200];
    rom[0x100] = 0x16; // LD D,$01
    rom[0x101] = 0x01;
    rom[0x102] = 0xCB; // RR D (carry in 0)
    rom[0x103] = 0x1A;
    rom[0x104] = 0x16; // LD D,$00
    rom[0x105] = 0x00;
    rom[0x106] = 0xCB; // RR D (carry in 1)
    rom[0x107] = 0x1A;

    let (mut cpu, mut bus) = boot_machine(rom);
    cpu.set_flag_c(false);

    step(&mut cpu, &mut bus).unwrap();
    step(&mut cpu, &mut bus).unwrap();
    assert_eq!(cpu.d, 0x00);
    assert!(cpu.flag_z());
    assert!(cpu.flag_c());
    assert!(!cpu.flag_n());
    assert!(!cpu.flag_h());

    step(&mut cpu, &mut bus).unwrap();
    // carry still 1 from previous RR
    step(&mut cpu, &mut bus).unwrap();
    assert_eq!(cpu.d, 0x80);
    assert!(!cpu.flag_z());
    assert!(!cpu.flag_c());
}

#[test]
fn rr_hl_read_modify_write() {
    let mut rom = vec![0x00; 0x200];
    rom[0x100] = 0x21; // LD HL,$C000
    rom[0x101] = 0x00;
    rom[0x102] = 0xC0;
    rom[0x103] = 0xCB; // RR (HL)
    rom[0x104] = 0x1E;

    let (mut cpu, mut bus) = boot_machine(rom);
    bus.write8(0xC000, 0x01);
    cpu.set_flag_c(true);
    step(&mut cpu, &mut bus).unwrap();
    step(&mut cpu, &mut bus).unwrap();
    assert_eq!(bus.read8(0xC000), 0x80);
    assert!(cpu.flag_c());
}

#[test]
fn bit_sets_znh_preserves_carry() {
    let mut rom = vec![0x00; 0x200];
    rom[0x100] = 0x16; // LD D,$01
    rom[0x101] = 0x01;
    rom[0x102] = 0xCB; // BIT 0,D
    rom[0x103] = 0x42;
    rom[0x104] = 0x16; // LD D,$FE
    rom[0x105] = 0xFE;
    rom[0x106] = 0xCB; // BIT 0,D
    rom[0x107] = 0x42;

    let (mut cpu, mut bus) = boot_machine(rom);
    cpu.set_flag_c(true);

    step(&mut cpu, &mut bus).unwrap(); // LD D,$01
    step(&mut cpu, &mut bus).unwrap(); // BIT 0,D — bit set
    assert!(!cpu.flag_z());
    assert!(!cpu.flag_n());
    assert!(cpu.flag_h());
    assert!(cpu.flag_c());

    step(&mut cpu, &mut bus).unwrap(); // LD D,$FE
    step(&mut cpu, &mut bus).unwrap(); // BIT 0,D — bit clear
    assert!(cpu.flag_z());
    assert!(!cpu.flag_n());
    assert!(cpu.flag_h());
    assert!(cpu.flag_c());
}

#[test]
fn bit_hl_reads_memory() {
    let mut rom = vec![0x00; 0x200];
    rom[0x100] = 0x21; // LD HL,$C000
    rom[0x101] = 0x00;
    rom[0x102] = 0xC0;
    rom[0x103] = 0xCB; // BIT 7,(HL)
    rom[0x104] = 0x7E;

    let (mut cpu, mut bus) = boot_machine(rom);
    bus.write8(0xC000, 0x80);
    step(&mut cpu, &mut bus).unwrap();
    step(&mut cpu, &mut bus).unwrap();
    assert!(!cpu.flag_z());
    assert!(cpu.flag_h());
}

#[test]
fn jp_hl_sets_pc_to_hl() {
    let mut rom = vec![0x00; 0x200];
    rom[0x100] = 0x21; // LD HL,$0150
    rom[0x101] = 0x50;
    rom[0x102] = 0x01;
    rom[0x103] = 0xE9; // JP (HL)
    rom[0x150] = 0x00; // landing NOP

    let (mut cpu, mut bus) = boot_machine(rom);
    let flags_before = cpu.f();
    step(&mut cpu, &mut bus).unwrap(); // LD HL
    step(&mut cpu, &mut bus).unwrap(); // JP (HL)
    assert_eq!(cpu.pc, 0x0150);
    assert_eq!(cpu.f(), flags_before);
}

#[test]
fn add_a_a_via_step() {
    let mut rom = vec![0x00; 0x200];
    rom[0x100] = 0x3E; // LD A,$40
    rom[0x101] = 0x40;
    rom[0x102] = 0x87; // ADD A,A

    let (mut cpu, mut bus) = boot_machine(rom);
    step(&mut cpu, &mut bus).unwrap();
    step(&mut cpu, &mut bus).unwrap();
    assert_eq!(cpu.a, 0x80);
    assert!(!cpu.flag_z());
    assert!(!cpu.flag_n());
    assert!(!cpu.flag_h());
    assert!(!cpu.flag_c());
}

#[test]
fn rlca_clears_z_even_when_result_is_zero() {
    // Non-CB RLCA: Z always 0. CB RLC A: Z=1 when A becomes 0.
    let mut rom = vec![0x00; 0x200];
    rom[0x100] = 0xAF; // XOR A  → A=0, Z=1
    rom[0x101] = 0x07; // RLCA
    rom[0x102] = 0x3E; // LD A,$80
    rom[0x103] = 0x80;
    rom[0x104] = 0x07; // RLCA → A=$01, C=1
    rom[0x105] = 0xAF; // XOR A
    rom[0x106] = 0xCB; // RLC A
    rom[0x107] = 0x07;

    let (mut cpu, mut bus) = boot_machine(rom);
    step(&mut cpu, &mut bus).unwrap();
    assert!(cpu.flag_z());
    step(&mut cpu, &mut bus).unwrap(); // RLCA on 0
    assert_eq!(cpu.a, 0x00);
    assert!(!cpu.flag_z(), "RLCA must clear Z even when A is 0");
    assert!(!cpu.flag_n());
    assert!(!cpu.flag_h());
    assert!(!cpu.flag_c());

    step(&mut cpu, &mut bus).unwrap(); // LD A,$80
    step(&mut cpu, &mut bus).unwrap(); // RLCA
    assert_eq!(cpu.a, 0x01);
    assert!(!cpu.flag_z());
    assert!(cpu.flag_c());

    step(&mut cpu, &mut bus).unwrap(); // XOR A
    assert!(cpu.flag_z());
    step(&mut cpu, &mut bus).unwrap(); // CB RLC A
    assert_eq!(cpu.a, 0x00);
    assert!(cpu.flag_z(), "CB RLC A sets Z when result is 0");
    assert!(!cpu.flag_c());
}

#[test]
fn rla_rra_rrca_through_carry() {
    let mut rom = vec![0x00; 0x200];
    rom[0x100] = 0x3E; // LD A,$80
    rom[0x101] = 0x80;
    rom[0x102] = 0x37; // SCF
    rom[0x103] = 0x17; // RLA → A=$01, C=1 (bit7 out; old C into bit0)
    rom[0x104] = 0x1F; // RRA → A=$80, C=1
    rom[0x105] = 0x0F; // RRCA → A=$40, C=0

    let (mut cpu, mut bus) = boot_machine(rom);
    step(&mut cpu, &mut bus).unwrap();
    step(&mut cpu, &mut bus).unwrap();
    assert!(cpu.flag_c());
    step(&mut cpu, &mut bus).unwrap(); // RLA
    assert_eq!(cpu.a, 0x01);
    assert!(cpu.flag_c());
    assert!(!cpu.flag_z());
    step(&mut cpu, &mut bus).unwrap(); // RRA
    assert_eq!(cpu.a, 0x80);
    assert!(cpu.flag_c());
    assert!(!cpu.flag_z());
    step(&mut cpu, &mut bus).unwrap(); // RRCA
    assert_eq!(cpu.a, 0x40);
    assert!(!cpu.flag_c());
    assert!(!cpu.flag_z());
}

#[test]
fn cb_shifts_reg_and_mem_hl() {
    // Ridge @ $58BD bank $1F was CB 3F (SRL A).
    let mut rom = vec![0x00; 0x200];
    rom[0x100] = 0x3E; // LD A,$81
    rom[0x101] = 0x81;
    rom[0x102] = 0xCB; // SRL A
    rom[0x103] = 0x3F;
    rom[0x104] = 0x3E; // LD A,$81
    rom[0x105] = 0x81;
    rom[0x106] = 0xCB; // SRA A
    rom[0x107] = 0x2F;
    rom[0x108] = 0x21; // LD HL,$C000
    rom[0x109] = 0x00;
    rom[0x10A] = 0xC0;
    rom[0x10B] = 0x36; // LD (HL),$40
    rom[0x10C] = 0x40;
    rom[0x10D] = 0xCB; // SLA (HL)
    rom[0x10E] = 0x26;

    let (mut cpu, mut bus) = boot_machine(rom);
    step(&mut cpu, &mut bus).unwrap();
    step(&mut cpu, &mut bus).unwrap(); // SRL A
    assert_eq!(cpu.a, 0x40);
    assert!(cpu.flag_c());
    assert!(!cpu.flag_z());
    assert!(!cpu.flag_n());
    assert!(!cpu.flag_h());

    step(&mut cpu, &mut bus).unwrap();
    step(&mut cpu, &mut bus).unwrap(); // SRA A
    assert_eq!(cpu.a, 0xC0);
    assert!(cpu.flag_c());
    assert!(!cpu.flag_z());

    step(&mut cpu, &mut bus).unwrap();
    step(&mut cpu, &mut bus).unwrap();
    step(&mut cpu, &mut bus).unwrap(); // SLA (HL)
    assert_eq!(bus.read8(0xC000), 0x80);
    assert!(!cpu.flag_c());
    assert!(!cpu.flag_z());
    assert!(!cpu.flag_n());
    assert!(!cpu.flag_h());
}

#[test]
fn wram_clear_loop_fills_via_hl_bc() {
    // LD HL,$C000 / LD BC,$0004 / then the Red clear body.
    let mut rom = vec![0x00; 0x200];
    rom[0x100] = 0x21; // LD HL,$C000
    rom[0x101] = 0x00;
    rom[0x102] = 0xC0;
    rom[0x103] = 0x01; // LD BC,$0004
    rom[0x104] = 0x04;
    rom[0x105] = 0x00;
    rom[0x106] = 0x36; // LD (HL),$00
    rom[0x107] = 0x00;
    rom[0x108] = 0x23; // INC HL
    rom[0x109] = 0x0B; // DEC BC
    rom[0x10A] = 0x78; // LD A,B
    rom[0x10B] = 0xB1; // OR C
    rom[0x10C] = 0x20; // JR NZ,$106
    rom[0x10D] = 0xF8;
    rom[0x10E] = 0x00; // done

    let (mut cpu, mut bus) = boot_machine(rom);
    bus.write8(0xC000, 0xAA);
    bus.write8(0xC003, 0xBB);

    for _ in 0..40 {
        step(&mut cpu, &mut bus).unwrap();
        if cpu.pc == 0x010E {
            break;
        }
    }
    assert_eq!(cpu.pc, 0x010E);
    assert_eq!(cpu.read_r16(Reg16::Hl), 0xC004);
    assert_eq!(cpu.read_r16(Reg16::Bc), 0x0000);
    assert_eq!(bus.read8(0xC000), 0x00);
    assert_eq!(bus.read8(0xC003), 0x00);
}

#[test]
fn res_clears_bit_without_flags() {
    let mut rom = vec![0x00; 0x200];
    rom[0x100] = 0x3E; // LD A,$81
    rom[0x101] = 0x81;
    rom[0x102] = 0xCB; // RES 0,A
    rom[0x103] = 0x87;

    let (mut cpu, mut bus) = boot_machine(rom);
    step(&mut cpu, &mut bus).unwrap();
    assert_eq!(cpu.a, 0x81);
    step(&mut cpu, &mut bus).unwrap();
    assert_eq!(cpu.a, 0x80);
    assert_eq!(cpu.pc, 0x0104);
}

#[test]
fn res_set_mem_hl_mutates_without_flags() {
    // Ridge at $58B6 bank $10: CB AE (RES 5,(HL)) then CB DE (SET 3,(HL)).
    let mut rom = vec![0x00; 0x200];
    rom[0x100] = 0x21; // LD HL,$C000
    rom[0x101] = 0x00;
    rom[0x102] = 0xC0;
    rom[0x103] = 0x36; // LD (HL),$FF
    rom[0x104] = 0xFF;
    rom[0x105] = 0xCB; // RES 5,(HL)
    rom[0x106] = 0xAE;
    rom[0x107] = 0xCB; // SET 3,(HL)
    rom[0x108] = 0xDE;

    let (mut cpu, mut bus) = boot_machine(rom);
    step(&mut cpu, &mut bus).unwrap();
    step(&mut cpu, &mut bus).unwrap();
    assert_eq!(bus.read8(0xC000), 0xFF);
    let flags = cpu.f();
    step(&mut cpu, &mut bus).unwrap();
    assert_eq!(bus.read8(0xC000), 0xDF); // bit 5 cleared
    assert_eq!(cpu.f(), flags);
    step(&mut cpu, &mut bus).unwrap();
    assert_eq!(bus.read8(0xC000), 0xDF); // bit 3 already set
    assert_eq!(cpu.f(), flags);
    assert_eq!(cpu.pc, 0x0109);
}

#[test]
fn ie_and_ly_readable() {
    let mut bus = Bus::from_rom(vec![0; 0x200]);
    bus.write8(0xFFFF, 0x00);
    assert_eq!(bus.read8(0xFFFF), 0x00);
    assert_eq!(bus.read8(0xFF44), 0x00);
}

#[test]
fn ld_abs_a_writes_wram_then_jp() {
    let mut rom = vec![0x00; 0x2000];
    rom[0x100] = 0xAF; // XOR A
    rom[0x101] = 0xEA; // LD ($CF1A), A
    rom[0x102] = 0x1A;
    rom[0x103] = 0xCF;
    rom[0x104] = 0xC3; // JP $1F54
    rom[0x105] = 0x54;
    rom[0x106] = 0x1F;
    rom[0x1F54] = 0xF3; // DI

    let (mut cpu, mut bus) = boot_machine(rom);
    step(&mut cpu, &mut bus).unwrap(); // XOR A
    assert_eq!(cpu.a, 0x00);
    step(&mut cpu, &mut bus).unwrap(); // LD ($CF1A), A
    assert_eq!(bus.read8(0xCF1A), 0x00);
    step(&mut cpu, &mut bus).unwrap(); // JP
    assert_eq!(cpu.pc, 0x1F54);
    step(&mut cpu, &mut bus).unwrap(); // DI
    assert!(!cpu.ime);
}

#[test]
fn cgb_armed_stop_switches_key1_and_covers_pause_without_extra_div() {
    use crate::hw::KEY1;
    use crate::timer::DIV;

    let mut rom = vec![0x00; 0x200];
    rom[0x100] = 0x10; // STOP
    rom[0x101] = 0x00;

    let (mut cpu, mut bus) = boot_cgb_machine(rom);
    bus.write8(KEY1, 0x01);
    step(&mut cpu, &mut bus).unwrap();
    assert_eq!(cpu.pc, 0x0102);
    assert_eq!(bus.read8(KEY1), 0xFE);
    assert_eq!(bus.read8(DIV), 0);
    bus.tick(252);
    assert_eq!(
        bus.read8(DIV),
        0,
        "STOP pause must include the instruction budget (no extra 4 T after unfreeze)"
    );
    bus.tick(4);
    assert_eq!(bus.read8(DIV), 1);
}

#[test]
fn dmg_stop_is_two_byte_nop() {
    use crate::hw::KEY1;

    let mut rom = vec![0x00; 0x200];
    rom[0x100] = 0x10;
    rom[0x101] = 0x00;
    rom[0x102] = 0x00;

    let (mut cpu, mut bus) = boot_machine(rom);
    step(&mut cpu, &mut bus).unwrap();
    assert_eq!(cpu.pc, 0x0102);
    assert_eq!(bus.read8(KEY1), 0xFF);
}
