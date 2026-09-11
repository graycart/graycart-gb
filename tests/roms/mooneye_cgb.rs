//! Targeted Mooneye CGB acceptance — classify before fixing.
//!
//! Fixtures live under `tests/fixtures/mooneye/` when vendored (same tree as
//! DMG acceptance). Upstream: [Mooneye Test Suite](https://github.com/Gekkio/mooneye-test-suite)
//! `misc/` (CGB extra) plus CGB-tagged bits. KEY1 / HDMA / CRAM / priority are
//! **not** a full Mooneye folder; those stay commercial smoke + unit tests until
//! dedicated ROMs are added.
//!
//! Default `cargo test` only prints the classification. The ignored matrix runs
//! present ROMs with [`HostHardwarePref::GameBoyColor`].

use crate::harness::{Outcome, RomLaunchMode, load_machine_launched, run_mooneye_launched};
use graycart::step;
use std::path::{Path, PathBuf};

const FIXTURE_ROOT: &str = "tests/fixtures/mooneye";
const STEP_LIMIT: u32 = 5_000_000;

/// (relative path, hardware area, launch mode the ROM actually inspects)
const SUITE: &[(&str, &str, RomLaunchMode)] = &[
    (
        "misc/bits/unused_hwio-C.gb",
        "CGB unused HWIO",
        RomLaunchMode::FastCgb,
    ),
    (
        "misc/boot_hwio-C.gb",
        "CGB boot HWIO",
        RomLaunchMode::FastCgb,
    ),
    (
        "misc/boot_regs-cgb.gb",
        "CGB boot registers A=$11",
        RomLaunchMode::FastCgb,
    ),
    (
        "misc/ppu/vblank_stat_intr-C.gb",
        "CGB STAT/VBlank",
        RomLaunchMode::FastCgb,
    ),
];

fn fixture(rel: &str) -> PathBuf {
    Path::new(FIXTURE_ROOT).join(rel)
}

fn print_row(name: &str, area: &str, outcome: &str, detail: &str) {
    if detail.is_empty() {
        eprintln!("{:<42} {:<28} {outcome}", name, area);
    } else {
        eprintln!("{:<42} {:<28} {outcome}  ({detail})", name, area);
    }
}

#[test]
fn mooneye_cgb_suite_classified() {
    eprintln!();
    eprintln!("{:<42} {:<28} status", "ROM", "area");
    let mut missing = 0usize;
    let mut present = 0usize;
    for (rel, area, _) in SUITE {
        let path = fixture(rel);
        if path.exists() {
            present += 1;
            print_row(rel, area, "PRESENT", "not run here; use ignored matrix");
        } else {
            missing += 1;
            print_row(rel, area, "MISSING_FIXTURE", "do not treat as PASS");
        }
    }
    eprintln!(
        "classified: {} listed, {present} present, {missing} missing — no silent skip-as-pass",
        SUITE.len()
    );
}

#[test]
#[ignore = "Mooneye CGB matrix; vendor misc/ fixtures; run with --ignored --nocapture"]
fn mooneye_cgb_acceptance_matrix() {
    eprintln!();
    eprintln!("{:<42} {:<28} result", "ROM", "area");
    let mut counts = [0usize; 4];
    for (rel, area, mode) in SUITE {
        let path = fixture(rel);
        let outcome = if path.exists() {
            run_mooneye_launched(&path, STEP_LIMIT, *mode)
        } else {
            Outcome::Unsupported("missing fixture".into())
        };
        match &outcome {
            Outcome::Pass => counts[0] += 1,
            Outcome::Fail(_) => counts[1] += 1,
            Outcome::Timeout => counts[2] += 1,
            Outcome::Unsupported(_) => counts[3] += 1,
        }
        print_row(rel, area, outcome.label(), outcome.detail());
    }
    eprintln!(
        "PASS={} FAIL={} TIMEOUT={} UNSUPPORTED={}",
        counts[0], counts[1], counts[2], counts[3]
    );
}

fn dump_cgb_mmio(bus: &graycart::Bus, cpu: &graycart::Cpu, label: &str) {
    eprintln!("=== {label} ===");
    eprintln!(
        "PC={:04X} A={:02X} BC={:02X}{:02X} DE={:02X}{:02X} HL={:02X}{:02X} halted={} IME={}",
        cpu.pc, cpu.a, cpu.b, cpu.c, cpu.d, cpu.e, cpu.h, cpu.l, cpu.halted, cpu.ime
    );
    let ports = [
        0xFF0F, 0xFF12, 0xFF41, 0xFF4C, 0xFF4D, 0xFF4F, 0xFF51, 0xFF52, 0xFF53, 0xFF54, 0xFF55,
        0xFF68, 0xFF69, 0xFF6A, 0xFF6B, 0xFF70, 0xFF72, 0xFF73, 0xFF74, 0xFF75, 0xFF76, 0xFF77,
        0xFFFF,
    ];
    for addr in ports {
        eprint!("  ${addr:04X}={:02X}", bus.read8(addr));
        if addr == 0xFF41 {
            eprint!(" (STAT mode={})", bus.read8(addr) & 3);
        }
        eprintln!();
    }
    eprintln!(
        "  LY={:02X} LCDC={:02X} IE={:02X} IF={:02X}",
        bus.read8(0xFF44),
        bus.read8(0xFF40),
        bus.read8(0xFFFF),
        bus.read8(0xFF0F)
    );
}

/// Same sequence as Gekkio `unused_hwio-C.s` (Pan Docs unused bits = 1).
/// First mismatch is the silicon/harness bug to research — do not "fix" to PASS.
fn unused_hwio_cases() -> &'static [(u16, u8, u8, u8, &'static str)] {
    &[
        (0xFF00, 0xC0, 0xFF, 0xC0, "P1"),
        (0xFF00, 0xC0, 0x3F, 0xC0, "P1"),
        (0xFF02, 0x7E, 0x7E, 0x7E, "SC"),
        (0xFF02, 0x7E, 0x00, 0x7E, "SC"),
        (0xFF07, 0xF8, 0xF8, 0xF8, "TAC"),
        (0xFF07, 0xF8, 0x00, 0xF8, "TAC"),
        (0xFF0F, 0xE0, 0xE0, 0xE0, "IF"),
        (0xFF0F, 0xE0, 0x00, 0xE0, "IF"),
        (0xFF41, 0x80, 0x80, 0x80, "STAT"),
        (0xFF41, 0x80, 0x00, 0x80, "STAT"),
        (0xFF10, 0x80, 0x00, 0x80, "NR10"),
        (0xFF10, 0x80, 0x80, 0x80, "NR10"),
        (0xFF1A, 0x7F, 0x00, 0x7F, "NR30"),
        (0xFF1A, 0x7F, 0x7F, 0x7F, "NR30"),
        (0xFF1C, 0x9F, 0x00, 0x9F, "NR32"),
        (0xFF1C, 0x9F, 0x9F, 0x9F, "NR32"),
        (0xFF20, 0xC0, 0x00, 0xC0, "NR41"),
        (0xFF20, 0xC0, 0xC0, 0xC0, "NR41"),
        (0xFF23, 0x3F, 0x00, 0x3F, "NR44"),
        (0xFF23, 0x3F, 0x3F, 0x3F, "NR44"),
        (0xFF26, 0x70, 0x80, 0x70, "NR52"),
        (0xFF26, 0x70, 0xF0, 0x70, "NR52"),
        (0xFFFF, 0xE0, 0x00, 0x00, "IE"),
        (0xFFFF, 0xE0, 0xE0, 0xE0, "IE"),
        (0xFF4F, 0xFE, 0x00, 0xFE, "VBK"),
        (0xFF4F, 0xFE, 0xFE, 0xFE, "VBK"),
        (0xFF68, 0x40, 0x00, 0x40, "BGPI"),
        (0xFF68, 0x40, 0x40, 0x40, "BGPI"),
        (0xFF6A, 0x40, 0x00, 0x40, "BGPI/OBPI"),
        (0xFF6A, 0x40, 0x40, 0x40, "OBPI"),
        (0xFF72, 0xFF, 0x00, 0x00, "FF72"),
        (0xFF72, 0xFF, 0xFF, 0xFF, "FF72"),
        (0xFF73, 0xFF, 0x00, 0x00, "FF73"),
        (0xFF73, 0xFF, 0xFF, 0xFF, "FF73"),
        (0xFF75, 0xFF, 0x00, 0x8F, "FF75"),
        (0xFF75, 0xFF, 0xFF, 0xFF, "FF75"),
        (0xFF76, 0xFF, 0x00, 0x00, "FF76"),
        (0xFF76, 0xFF, 0xFF, 0x00, "FF76"),
        (0xFF77, 0xFF, 0x00, 0x00, "FF77"),
        (0xFF77, 0xFF, 0xFF, 0x00, "FF77"),
        (0xFF03, 0xFF, 0x00, 0xFF, "unmapped FF03"),
        (0xFF03, 0xFF, 0xFF, 0xFF, "unmapped FF03"),
        (0xFF4C, 0xFF, 0x00, 0xFF, "unmapped KEY0"),
        (0xFF4C, 0xFF, 0xFF, 0xFF, "unmapped KEY0"),
        (0xFF4D, 0xFF, 0x00, 0xFF, "unmapped KEY1 (test)"),
        (0xFF4D, 0xFF, 0xFF, 0xFF, "unmapped KEY1 (test)"),
        (0xFF69, 0xFF, 0x00, 0xFF, "unmapped BGPD (compat)"),
        (0xFF69, 0xFF, 0xFF, 0xFF, "unmapped BGPD (compat)"),
        (0xFF6B, 0xFF, 0x00, 0xFF, "unmapped OBPD (test)"),
        (0xFF6B, 0xFF, 0xFF, 0xFF, "unmapped OBPD (test)"),
    ]
}

#[test]
#[ignore = "Mooneye CGB first-fail MMIO dump"]
fn mooneye_cgb_first_fail_mmio_dump() {
    let path = fixture("misc/bits/unused_hwio-C.gb");
    let (cpu, mut bus) =
        load_machine_launched(&path, RomLaunchMode::FastCgb).expect("load unused_hwio-C");
    dump_cgb_mmio(&bus, &cpu, "unused_hwio-C after FastCgb");

    let mut first = None;
    for (addr, mask, write, expect, name) in unused_hwio_cases() {
        bus.write8(*addr, *write);
        let got = bus.read8(*addr) & mask;
        let want = expect & mask;
        if got != want {
            first = Some((*addr, *name, *write, *mask, want, got));
            break;
        }
    }
    match first {
        Some((addr, name, write, mask, want, got)) => {
            eprintln!(
                "FIRST unused_hwio-C mismatch: {name} ${addr:04X} write={write:02X} mask={mask:02X} expect={want:02X} got={got:02X}"
            );
            dump_cgb_mmio(&bus, &cpu, "after first unused_hwio write/read");
        }
        None => eprintln!(
            "Rust-side unused_hwio subset all matched (ROM may still fail later unmapped rows)"
        ),
    }

    let boot = fixture("misc/boot_hwio-C.gb");
    let (cpu, bus) =
        load_machine_launched(&boot, RomLaunchMode::FastCgb).expect("load boot_hwio-C");
    dump_cgb_mmio(
        &bus,
        &cpu,
        "boot_hwio-C after FastCgb (compat KEY1 must be $FF per Pan Docs footnote 8)",
    );
    let table_ff4d = 0xFFu8;
    let table_ff4f = 0xFEu8;
    let table_ff68 = 0xC8u8;
    eprintln!(
        "boot_hwio table vs Fast: KEY1 expect {table_ff4d:02X} got {:02X}; VBK expect {table_ff4f:02X} got {:02X}; BGPI expect {table_ff68:02X} got {:02X}",
        bus.read8(0xFF4D),
        bus.read8(0xFF4F),
        bus.read8(0xFF68)
    );

    for (rel, tag, mode) in [
        (
            "misc/bits/unused_hwio-C.gb",
            "unused_hwio-C",
            RomLaunchMode::FastCgb,
        ),
        ("misc/boot_hwio-C.gb", "boot_hwio-C", RomLaunchMode::FastCgb),
        (
            "misc/ppu/vblank_stat_intr-C.gb",
            "vblank_stat_intr-C",
            RomLaunchMode::FastCgb,
        ),
    ] {
        let path = fixture(rel);
        let (mut cpu, mut bus) = load_machine_launched(&path, mode).unwrap();
        let mut same_pc = 0u32;
        let mut last_pc = cpu.pc;
        let mut outcome = "TIMEOUT";
        for _ in 0..STEP_LIMIT {
            if step(&mut cpu, &mut bus).is_err() {
                outcome = "FAULT";
                break;
            }
            if cpu.b == 3 && cpu.c == 5 && cpu.d == 8 && cpu.e == 13 && cpu.h == 21 && cpu.l == 34 {
                outcome = "PASS";
                break;
            }
            if cpu.pc == last_pc {
                same_pc += 1;
            } else {
                same_pc = 0;
                last_pc = cpu.pc;
            }
            if !cpu.halted && same_pc >= 64 {
                outcome = "FAIL_LOOP";
                break;
            }
        }
        eprintln!("=== {tag} ROM {outcome} ===");
        dump_cgb_mmio(&bus, &cpu, tag);
        eprintln!(
            "  HRAM FF80-FF90={}",
            (0xFF80u16..=0xFF90)
                .map(|a| format!("{:02X}", bus.read8(a)))
                .collect::<Vec<_>>()
                .join(" ")
        );
        eprintln!("  serial={:?}", bus.serial_text());
        let _ = cpu;
    }
}

fn machine_t(bus: &graycart::Bus) -> u32 {
    u32::from(bus.ppu.ly()) * 456 + bus.ppu.line_cycles()
}

/// Gekkio `vblank_stat_intr-C` round 1: 54 NOPs after LY=$8F, DIV reset, EI, HALT.
/// ISR at `$01D8` samples DIV; hardware leaves B=`$01` (DIV increment before the read).
#[test]
fn vblank_round1_isr_samples_div_one() {
    let path = fixture("misc/ppu/vblank_stat_intr-C.gb");
    assert!(path.exists(), "vendor vblank_stat_intr-C.gb");
    let (mut cpu, mut bus) = load_machine_launched(&path, RomLaunchMode::FastCgb).unwrap();

    const DIV_WRITE: u16 = 0x01C9; // ldh [$FF04], a
    const ISR_DIV: u16 = 0x01D8;

    let mut t0 = None;
    let mut t6 = None;
    let mut sample = None;
    let mut last_if0 = bus.read8(0xFF0F) & 1;
    let mut t_if0 = None;

    for i in 0..STEP_LIMIT {
        let pc = cpu.pc;
        let t = machine_t(&bus);
        let if0 = bus.read8(0xFF0F) & 1;
        if t_if0.is_none() && t0.is_some() && last_if0 == 0 && if0 == 1 {
            t_if0 = Some((t, pc, bus.ppu.ly(), bus.ppu.line_cycles(), cpu.halted));
        }
        last_if0 = if0;

        if pc == DIV_WRITE && t0.is_none() {
            let _ = step(&mut cpu, &mut bus).unwrap();
            t0 = Some((
                machine_t(&bus),
                bus.read8(0xFF04),
                bus.ppu.ly(),
                bus.ppu.line_cycles(),
            ));
            continue;
        }
        if pc == ISR_DIV {
            let t_before = t;
            let div_before = bus.read8(0xFF04);
            let _ = step(&mut cpu, &mut bus).unwrap();
            sample = Some(cpu.a);
            t6 = Some((t_before, machine_t(&bus), div_before, cpu.a));
            break;
        }

        if step(&mut cpu, &mut bus).is_err() {
            panic!("fault at step {i} PC=${pc:04X}");
        }
        if !cpu.halted && cpu.pc == pc && i > 1000 && cpu.b == 0x42 {
            break;
        }
    }

    eprintln!("T0 DIV write done: {t0:?}");
    eprintln!("T3 IF.0 rise: {t_if0:?}");
    eprintln!("T6 ISR DIV: {t6:?}");
    if let (Some((t0, _, _, _)), Some((t6b, _, _, _))) = (t0, t6) {
        eprintln!(
            "T6-T0 (machine T at ISR fetch vs after DIV write) = {}",
            t6b.saturating_sub(t0)
        );
    }

    assert_eq!(
        sample,
        Some(1),
        "round1 ISR DIV sample: T0={t0:?} T3={t_if0:?} T6={t6:?}"
    );
}

/// Gekkio `boot_hwio-C`: mismatch handler is `JP NZ,$01DF` while HL still names the port.
#[test]
fn boot_hwio_c_first_scan_mismatch() {
    let path = fixture("misc/boot_hwio-C.gb");
    assert!(path.exists(), "vendor boot_hwio-C.gb");
    let (mut cpu, mut bus) = load_machine_launched(&path, RomLaunchMode::FastCgb).unwrap();

    const MISMATCH: u16 = 0x01DF;
    let mut ff12_log: Vec<(u16, u8, u16)> = Vec::new();
    let mut hit = None;

    for i in 0..STEP_LIMIT {
        if cpu.hl() == 0xFF12 {
            ff12_log.push((cpu.pc, bus.read8(0xFF12), cpu.hl()));
        }
        if cpu.pc == MISMATCH {
            hit = Some((
                cpu.hl(),
                cpu.a,
                cpu.b,
                bus.read8(0xFF12),
                bus.read8(0xFF68),
                bus.read8(0xFF76),
            ));
            break;
        }
        if cpu.b == 3 && cpu.c == 5 && cpu.d == 8 && cpu.e == 13 && cpu.h == 21 && cpu.l == 34 {
            break;
        }
        if step(&mut cpu, &mut bus).is_err() {
            panic!("fault at step {i} PC=${:04X}", cpu.pc);
        }
        if cpu.b == 0x42 && cpu.d == 0x42 && !cpu.halted && i > 10_000 {
            break;
        }
    }

    eprintln!("FF12 samples while HL=$FF12 (pc, NR12, HL): {ff12_log:?}");
    eprintln!("mismatch JP $01DF: HL/expectA/gotB/FF12/BGPI/PCM12 = {hit:?}");
    assert!(
        hit.is_none(),
        "boot_hwio-C first mismatch: {hit:?} (NR12 stayed F3 while HL=$FF12)"
    );
}

/// unused_hwio-C HRAM: `test_addr`/`test_got`/`test_reg`/`test_mask` at `$FF80`.
#[test]
fn unused_hwio_c_first_reg_on_fail() {
    let path = fixture("misc/bits/unused_hwio-C.gb");
    assert!(path.exists(), "vendor unused_hwio-C.gb");
    let (mut cpu, mut bus) = load_machine_launched(&path, RomLaunchMode::FastCgb).unwrap();

    let mut fail = None;
    for i in 0..STEP_LIMIT {
        if cpu.b == 3 && cpu.c == 5 && cpu.d == 8 && cpu.e == 13 && cpu.h == 21 && cpu.l == 34 {
            break;
        }
        if cpu.b == 0x42 && cpu.d == 0x42 && !cpu.halted && i > 1_000 {
            fail = Some((
                bus.read8(0xFF83),
                bus.read8(0xFF82),
                bus.read8(0xFF84),
                cpu.pc,
            ));
            break;
        }
        if step(&mut cpu, &mut bus).is_err() {
            panic!("fault at step {i} PC=${:04X}", cpu.pc);
        }
    }

    eprintln!("unused_hwio-C FAIL test_reg/got/mask/pc = {fail:?} (None means PASS)");
    assert!(
        fail.is_none(),
        "unused_hwio-C first failing port $FF{:02X} got={:02X} mask={:02X} pc=${:04X}",
        fail.unwrap().0,
        fail.unwrap().1,
        fail.unwrap().2,
        fail.unwrap().3
    );
}
