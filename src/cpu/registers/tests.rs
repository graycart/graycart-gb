use super::*;

#[test]
fn after_boot_matches_dmg_handoff() {
    let cpu = Cpu::after_boot();
    assert_eq!(cpu.a, 0x01);
    assert_eq!(cpu.pc, 0x0100);
    assert_eq!(cpu.sp, 0xFFFE);
    assert_eq!(cpu.af(), 0x01B0);
    assert_eq!(cpu.bc(), 0x0013);
    assert_eq!(cpu.de(), 0x00D8);
    assert_eq!(cpu.hl(), 0x014D);
    // F = $B0 → Z=1 N=0 H=1 C=1
    assert!(cpu.flag_z());
    assert!(!cpu.flag_n());
    assert!(cpu.flag_h());
    assert!(cpu.flag_c());
}

#[test]
fn after_boot_cgb_matches_native_cgb_handoff() {
    let cpu = Cpu::after_boot_cgb();
    assert_eq!(cpu.a, 0x11);
    assert_eq!(cpu.pc, 0x0100);
    assert_eq!(cpu.sp, 0xFFFE);
    assert_eq!(cpu.af(), 0x1180);
    assert_eq!(cpu.bc(), 0x0000);
    assert_eq!(cpu.de(), 0xFF56);
    assert_eq!(cpu.hl(), 0x000D);
    // F = $80 → Z=1 N=0 H=0 C=0
    assert!(cpu.flag_z());
    assert!(!cpu.flag_n());
    assert!(!cpu.flag_h());
    assert!(!cpu.flag_c());
    assert!(!cpu.ime);
    assert!(!cpu.halted);
}

#[test]
fn after_boot_cgb_compat_matches_dmg_mode_handoff() {
    let cpu = Cpu::after_boot_cgb_compat();
    assert_eq!(cpu.a, 0x11);
    assert_eq!(cpu.e, 0x08);
    assert_eq!(cpu.pc, 0x0100);
    assert_eq!(cpu.sp, 0xFFFE);
    assert_eq!(cpu.af(), 0x1180);
    assert_eq!(cpu.bc(), 0x0000);
    assert_eq!(cpu.de(), 0x0008);
    assert_eq!(cpu.hl(), 0x007C);
    assert!(cpu.flag_z());
    assert!(!cpu.flag_n());
    assert!(!cpu.flag_h());
    assert!(!cpu.flag_c());
    assert!(!cpu.ime);
    assert!(!cpu.halted);
}

#[test]
fn register_pairs_match_pan_docs_hi_lo() {
    let mut cpu = Cpu::new();
    cpu.set_bc(0x1234);
    assert_eq!(cpu.b, 0x12);
    assert_eq!(cpu.c, 0x34);
    assert_eq!(cpu.bc(), 0x1234);

    cpu.set_de(0xABCD);
    assert_eq!(cpu.d, 0xAB);
    assert_eq!(cpu.e, 0xCD);

    cpu.set_hl(0x014D);
    assert_eq!(cpu.h, 0x01);
    assert_eq!(cpu.l, 0x4D);

    // AF: F low nibble is unused and forced to 0
    cpu.set_af(0x56AB);
    assert_eq!(cpu.a, 0x56);
    assert_eq!(cpu.f(), 0xA0);
    assert_eq!(cpu.af(), 0x56A0);
}

#[test]
fn flag_bits_match_pan_docs_positions() {
    let mut cpu = Cpu::new();
    cpu.set_flag_z(true);
    assert_eq!(cpu.f(), 0b1000_0000);
    cpu.set_flag_n(true);
    assert_eq!(cpu.f(), 0b1100_0000);
    cpu.set_flag_h(true);
    assert_eq!(cpu.f(), 0b1110_0000);
    cpu.set_flag_c(true);
    assert_eq!(cpu.f(), 0b1111_0000);

    cpu.set_flag_z(false);
    assert_eq!(cpu.f(), FLAG_N | FLAG_H | FLAG_C);
}
