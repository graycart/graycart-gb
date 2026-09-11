use super::*;
use crate::bus::Bus;
use crate::cpu::Cpu;
use crate::ppu::{LCDC, LCDC_AFTER_BOOT, LCDC_POWER_ON};

fn rom_with_valid_header() -> Vec<u8> {
    let mut rom = vec![0u8; 0x8000];
    rom[0x0100] = 0x00;
    rom[0x0101] = 0x18;
    rom[0x0102] = 0xFE;
    rom
}

/// Skip-boot must not mix `Cpu::after_boot()` with a power-on PPU.
/// `Bus::new` / `from_rom` leaves LCDC at [`LCDC_POWER_ON`]; only [`apply_fast`]
/// writes [`LCDC_AFTER_BOOT`] (`$91`, Pan Docs post-boot).
#[test]
fn after_boot_cpu_alone_leaves_lcdc_power_on() {
    let mut bus = Bus::from_rom(rom_with_valid_header());
    let mut cpu = Cpu::after_boot();
    assert_eq!(bus.read8(LCDC), LCDC_POWER_ON);
    apply_fast(&mut cpu, &mut bus);
    assert_eq!(bus.read8(LCDC), LCDC_AFTER_BOOT);
}

#[test]
fn apply_fast_matches_after_boot() {
    let mut bus = Bus::from_rom(rom_with_valid_header());
    let mut cpu = Cpu::new();
    apply_fast(&mut cpu, &mut bus);
    let expected = Cpu::after_boot();
    assert_eq!(cpu.pc, expected.pc);
    assert_eq!(cpu.a, expected.a);
    assert_eq!(cpu.af(), expected.af());
    assert_eq!(bus.read8(LCDC), LCDC_AFTER_BOOT);
    assert_ne!(bus.read8(crate::apu::NR52) & 0x80, 0);
    assert_eq!(bus.read8(crate::apu::NR50), 0x77);
    assert_eq!(bus.read8(crate::apu::NR51), 0xF3);
}

#[test]
fn apply_fast_on_dmg_sets_a_01() {
    let mut bus = Bus::from_rom(rom_with_valid_header());
    let mut cpu = Cpu::new();
    apply_fast(&mut cpu, &mut bus);
    assert_eq!(cpu.a, 0x01);
    assert!(!bus.boot_rom_active());
}

#[test]
fn apply_fast_native_cgb_handoff() {
    use crate::hw::{CgbExecutionMode, HardwareModel, KEY0};
    use crate::ppu::OPRI;

    let mut rom = rom_with_valid_header();
    rom[0x0143] = 0xC0;
    let mut bus = Bus::from_rom(rom).with_hardware_model(HardwareModel::Cgb {
        mode: CgbExecutionMode::NativeCgb,
    });
    let mut cpu = Cpu::new();
    apply_fast(&mut cpu, &mut bus);

    assert_eq!(cpu.a, 0x11);
    assert_eq!(
        bus.read8(0xFF12),
        0xF3,
        "Pan Docs post-boot NR12 after Fast native"
    );
    assert_eq!(bus.ppu.cram.rgb555_bg(0, 0), 0x7FFF);
    assert_eq!(bus.read8(KEY0), 0xC0);
    bus.write8(KEY0, 0x04);
    assert_eq!(bus.read8(KEY0), 0xC0, "Fast locks KEY0");
    assert_eq!(bus.read8(OPRI), 0x00);
    assert_eq!(bus.read8(LCDC), LCDC_AFTER_BOOT);
    assert!(!bus.boot_rom_active());
}

#[test]
fn apply_fast_cgb_compat_handoff() {
    use crate::hw::{CgbExecutionMode, HardwareModel, KEY0};
    use crate::ppu::OPRI;

    let mut bus = Bus::from_rom(rom_with_valid_header()).with_hardware_model(HardwareModel::Cgb {
        mode: CgbExecutionMode::DmgCompatibility,
    });
    let mut cpu = Cpu::new();
    apply_fast(&mut cpu, &mut bus);

    assert_eq!(cpu.a, 0x11);
    assert_eq!(
        bus.read8(KEY0),
        0xFF,
        "KEY0 is CGB Mode only (compat unmapped)"
    );
    assert_eq!(bus.read8(OPRI), 0xFF, "OPRI is CGB Mode only");
    assert_eq!(bus.read8(0xFF00), 0xFF, "CGB boot deselects P1 groups");
    assert_eq!(bus.read8(0xFF0F), 0xE1, "Pan Docs post-boot IF");
    assert_eq!(bus.read8(0xFF68), 0xC8, "boot_hwio-C BGPI");
    assert_eq!(bus.read8(0xFF6A), 0xD0, "boot_hwio-C OBPI");
    assert!(!bus.boot_rom_active());
}

#[test]
fn prepare_boot_fast_matches_apply_fast() {
    let rom = rom_with_valid_header();
    let mut bus_a = Bus::from_rom(rom.clone());
    let mut bus_b = Bus::from_rom(rom);
    let mut cpu_fast = Cpu::new();
    let mut cpu_prepare = Cpu::new();
    apply_fast(&mut cpu_fast, &mut bus_a);
    prepare_boot(BootMode::Fast, &mut cpu_prepare, &mut bus_b);
    assert_eq!(cpu_prepare.pc, cpu_fast.pc);
    assert_eq!(cpu_prepare.a, cpu_fast.a);
    assert_eq!(cpu_prepare.af(), cpu_fast.af());
}

#[test]
fn prepare_boot_rom_resets_cpu_to_power_on() {
    let mut bus = Bus::from_rom(rom_with_valid_header());
    let mut cpu = Cpu::after_boot();
    cpu.pc = 0xDEAD;
    prepare_boot(BootMode::BootRom, &mut cpu, &mut bus);
    let fresh = Cpu::new();
    assert_eq!(cpu.pc, fresh.pc);
    assert_eq!(cpu.af(), fresh.af());
}

// --- 10A.0 Task 1: boot logo diagnostic (classification only) ---

mod logo_diagnostic {
    use super::*;
    use crate::cart::{Cartridge, NINTENDO_LOGO};
    use crate::debug::ExecSession;
    use crate::ppu::{Framebuffer, LCDC, SCY, decode_tile_row};
    use std::fs;

    fn rl(val: u8, carry: u8) -> (u8, u8) {
        let new_carry = (val >> 7) & 1;
        let val = (val << 1) | carry;
        (val, new_carry)
    }

    /// Reference DMG boot-ROM logo decompression ($0095 / $0096).
    ///
    /// The loop uses `push bc` / `pop bc` so only one `rl c` persists per outer
    /// iteration (four bits consumed per call), matching dmg.asm / Pan Docs.
    fn decompress_nibble(c_init: u8, carry_init: u8) -> (u8, u8, u8) {
        let mut c = c_init;
        let mut a = 0u8;
        let mut carry = carry_init;
        for _ in 0..4 {
            let saved_c = c;
            (_, carry) = rl(c, carry);
            (a, carry) = rl(a, carry);
            (c, carry) = rl(saved_c, carry);
            (a, carry) = rl(a, carry);
        }
        (a, c, carry)
    }

    fn expected_logo_vram(logo: &[u8; 48]) -> [u8; 384] {
        let mut out = [0u8; 384];
        let mut hl = 0usize;
        let mut carry = 0u8;
        for &byte in logo {
            let (a1, c, carry1) = decompress_nibble(byte, carry);
            carry = carry1;
            out[hl] = a1;
            hl += 2;
            out[hl] = a1;
            hl += 2;
            let (a2, _, carry2) = decompress_nibble(c, carry);
            carry = carry2;
            out[hl] = a2;
            hl += 2;
            out[hl] = a2;
            hl += 2;
        }
        out
    }

    pub(super) fn load_boot_rom() -> Option<[u8; 256]> {
        const CANDIDATES: &[&str] = &["target/release/boot/dmg_boot.bin", "boot/dmg_boot.bin"];
        for path in CANDIDATES {
            let bytes = fs::read(path).ok()?;
            if bytes.len() == 256 {
                return bytes.try_into().ok();
            }
        }
        None
    }

    pub(super) fn load_tetris() -> Option<Cartridge> {
        Cartridge::load("carts/tetris.gb").ok()
    }

    fn vram_range(bus: &Bus, start: u16, len: usize) -> Vec<u8> {
        (0..len).map(|i| bus.read8(start + i as u16)).collect()
    }

    fn hex_prefix(bytes: &[u8], n: usize) -> String {
        bytes
            .iter()
            .take(n)
            .map(|b| format!("{:02X}", b))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Headless boot-logo investigation harness. Run with:
    /// `cargo test boot_logo_diagnostic_dump -- --ignored --nocapture`
    #[test]
    #[ignore = "10A.0 classification diagnostic — needs dmg_boot.bin + carts/tetris.gb"]
    fn boot_logo_diagnostic_dump() {
        let boot = load_boot_rom().expect("dmg_boot.bin not found");
        let cart = load_tetris().expect("carts/tetris.gb not found");
        assert!(cart.header.logo_ok);

        let mut bus = Bus::new(cart);
        bus.enable_boot_rom(&boot);
        let mut cpu = Cpu::new();
        prepare_boot(BootMode::BootRom, &mut cpu, &mut bus);
        let mut session = ExecSession::new();

        let mut post_decomp_vram: Option<Vec<u8>> = None;
        let mut post_decomp_logo: Option<Vec<u8>> = None;
        let mut after_first_logo_byte: Option<Vec<u8>> = None;
        let mut pre_decomp_vram: Option<Vec<u8>> = None;
        let mut lcd_on_vram: Option<Vec<u8>> = None;
        let mut lcd_on_frame = None;
        let mut at_0027_count = 0u32;
        let mut first_decomp_entry = false;

        for step in 0..2_000_000 {
            let pc = cpu.pc;
            if !first_decomp_entry && pc == 0x0095 {
                first_decomp_entry = true;
                println!(
                    "first $0095: A=${:02X} C=${:02X} flags Z={} C={}",
                    cpu.a,
                    cpu.c,
                    cpu.flag_z(),
                    cpu.flag_c()
                );
            }
            if pre_decomp_vram.is_none() && pc == 0x0021 {
                pre_decomp_vram = Some(vram_range(&bus, 0x8010, 16));
                println!(
                    "at $0021: LCDC=${:02X} lcd_on={} mode={:?} vram_access={} ly={}",
                    bus.read8(LCDC),
                    bus.ppu.lcd_enabled(),
                    bus.ppu.mode(),
                    bus.ppu.vram_cpu_accessible(),
                    bus.ppu.ly()
                );
            }
            if pc == 0x0027 {
                at_0027_count += 1;
                // $0027 is the inner copy loop; the second visit is after the first
                // logo byte has been expanded into VRAM ($8010+).
                if at_0027_count == 2 {
                    after_first_logo_byte = Some(vram_range(&bus, 0x8010, 16));
                }
            }
            if post_decomp_vram.is_none() && pc == 0x0034 {
                post_decomp_vram = Some(vram_range(&bus, 0x8010, 384));
                post_decomp_logo = Some(vram_range(&bus, 0x0104, 48));
            }
            if bus.ppu.lcd_enabled() && lcd_on_vram.is_none() && bus.ppu.ly() > 0 {
                lcd_on_frame = Some(session.frames);
                lcd_on_vram = Some(vram_range(&bus, 0x8010, 384));
            }

            match session.step(&mut cpu, &mut bus) {
                Ok(_) => {}
                Err(e) => panic!("boot fault at step {step} pc=${pc:04X}: {e:?}"),
            }
            if bus.ppu.take_frame_ready() {
                session.note_frame();
            }
            if session.frames >= 120 {
                break;
            }
        }

        let lcdc = bus.read8(LCDC);
        let scy = bus.read8(SCY);
        let expected = expected_logo_vram(&NINTENDO_LOGO);
        let actual = vram_range(&bus, 0x8010, 384);
        let mismatches: Vec<_> = expected
            .iter()
            .zip(actual.iter())
            .enumerate()
            .filter(|(_, (e, a))| e != a)
            .take(16)
            .collect();

        println!("=== boot logo diagnostic ===");
        println!(
            "steps={} frames={} pc=${:04X} lcd_on_frame={:?}",
            session.steps, session.frames, cpu.pc, lcd_on_frame
        );
        println!(
            "LCDC=${:02X} SCY={} boot_active={}",
            lcdc,
            scy,
            bus.boot_rom_active()
        );
        println!("cart logo $0104+: {}", hex_prefix(&NINTENDO_LOGO[..], 16));
        if let Some(v) = &post_decomp_logo {
            println!("bus read  $0104+ at decomp end: {}", hex_prefix(v, 16));
        }
        println!("expected VRAM $8010+: {}", hex_prefix(&expected, 32));
        if let Some(v) = &pre_decomp_vram {
            println!("pre-decomp VRAM $8010+: {}", hex_prefix(v, 16));
        }
        if let Some(v) = &after_first_logo_byte {
            println!("after 1st logo byte VRAM $8010+: {}", hex_prefix(v, 16));
        }
        if let Some(v) = &post_decomp_vram {
            println!("post-decomp VRAM $8010+: {}", hex_prefix(v, 32));
        }
        if let Some(v) = &lcd_on_vram {
            println!("lcd-on     VRAM $8010+: {}", hex_prefix(v, 32));
        }
        println!("frame-120  VRAM $8010+: {}", hex_prefix(&actual, 32));
        println!(
            "VRAM logo region mismatches (first 16): {}",
            mismatches
                .iter()
                .map(|(i, (e, a))| format!("+{i:03X} exp={e:02X} act={a:02X}"))
                .collect::<Vec<_>>()
                .join(", ")
        );

        // Tilemap around $9904 (boot logo placement)
        let map = vram_range(&bus, 0x9900, 48);
        println!("tilemap $9900+: {}", hex_prefix(&map, 48));

        // Framebuffer: sample logo band (roughly center-top)
        let fb: &Framebuffer = &bus.ppu.framebuffer;
        let mut row48 = String::new();
        for x in 48..112 {
            row48.push_str(&format!("{}", fb.pixel(x, 48).index()));
        }
        println!("fb row 48 shades[48..112]: {}", row48);

        // Decode first tile row from actual VRAM vs expected
        let act_row = decode_tile_row(actual[0], actual[1]);
        let exp_row = decode_tile_row(expected[0], expected[1]);
        println!("decode tile0 row0 actual: {:?}", act_row);
        println!("decode tile0 row0 expect: {:?}", exp_row);

        // Also compare if we render BG manually from actual VRAM at LCDC state
        println!(
            "LCDC bits: BG={} map={} tile_data={}",
            lcdc & 1,
            (lcdc >> 3) & 1,
            (lcdc >> 4) & 1
        );

        // Classification hints printed for human review (test always passes)
        let vram_ok = mismatches.is_empty();
        let decode_ok = act_row == exp_row;
        println!(
            "classification hints: vram_transform_ok={} decode_row_ok={} fb_nonzero={}",
            vram_ok,
            decode_ok,
            row48.contains('1') || row48.contains('2') || row48.contains('3')
        );
    }

    /// Hypothesis check: power-on PPU must have LCD off so boot ROM can write VRAM.
    #[test]
    #[ignore = "10A.0 classification — needs dmg_boot.bin + carts/tetris.gb"]
    fn boot_logo_vram_with_lcd_forced_off_at_start() {
        let boot = load_boot_rom().expect("dmg_boot.bin");
        let cart = load_tetris().expect("tetris.gb");
        let mut bus = Bus::new(cart);
        bus.ppu.write(LCDC, 0x00); // LCD off before boot firmware runs
        bus.enable_boot_rom(&boot);
        let mut cpu = Cpu::new();
        prepare_boot(BootMode::BootRom, &mut cpu, &mut bus);
        let mut session = ExecSession::new();
        let expected = expected_logo_vram(&NINTENDO_LOGO);

        let mut saw_decomp = false;
        for _ in 0..500_000 {
            let pc = cpu.pc;
            if !saw_decomp && pc == 0x0095 {
                saw_decomp = true;
                println!(
                    "forced-off first $0095: A=${:02X} flags C={}",
                    cpu.a,
                    cpu.flag_c()
                );
            }
            if pc == 0x0034 {
                let actual = vram_range(&bus, 0x8010, 384);
                let mismatches = expected
                    .iter()
                    .zip(actual.iter())
                    .filter(|(e, a)| e != a)
                    .count();
                println!(
                    "LCD forced off: post-decomp mismatches={} first16 actual={}",
                    mismatches,
                    hex_prefix(&actual, 16)
                );
                println!("first16 expected={}", hex_prefix(&expected, 16));
                assert_eq!(
                    mismatches, 0,
                    "with LCD off, boot-ROM logo transform should match reference"
                );
                return;
            }
            session.step(&mut cpu, &mut bus).unwrap();
            if bus.ppu.take_frame_ready() {
                session.note_frame();
            }
        }
        panic!("never reached post-decomp");
    }

    #[test]
    #[ignore = "10A.0 diagnostic — needs carts/tetris.gb"]
    fn fast_boot_framebuffer_not_logo_specific() {
        let cart = load_tetris().expect("carts/tetris.gb");
        let mut bus = Bus::new(cart);
        let mut cpu = Cpu::new();
        apply_fast(&mut cpu, &mut bus);
        let mut session = ExecSession::new();
        while session.frames < 2 {
            session.step(&mut cpu, &mut bus).unwrap();
            if bus.ppu.take_frame_ready() {
                session.note_frame();
            }
        }
        let fb = &bus.ppu.framebuffer;
        let shade = fb.pixel(80, 72);
        println!("fast boot center pixel shade={}", shade.index());
        // Fast boot skips logo — any stable render is fine; just document.
        assert_ne!(bus.read8(LCDC), 0);
    }
}

// --- 10A.0 Task 4: synthetic boot Shade-region fixture ---
//
// Graycart-owned 256-byte overlay (not Nintendo firmware). LCD stays off while
// VRAM is filled, then LCDC=$91 / BGP=$E4 and JR-forever.
//
// 10A.0 acceptance notes (this slice):
// [x] Synthetic region test green (cargo test synthetic_boot_renders_known_shade_rectangle)
// [x] External DMG boot: Nintendo logo correct — human visual (2026-09-09)
// [x] External boot reaches game cleanly after logo — human
// [x] Fast / Skip Boot: game start OK — human
// [x] No HardwareModel / CGB palette path in this task
// Do not flip AGENTS.md 10A.0 to [x] until the human smoke rows are done.

mod synthetic_boot_render {
    use super::*;
    use crate::debug::{ExecSession, RunOutcome};
    use crate::ppu::{Framebuffer, LCDC, Shade, decode_tile_row, shade_from_bgp};

    /// Tile 1 bitplanes: Pan Docs sample row `$3C $7E` on every row.
    const TILE_LO: u8 = 0x3C;
    const TILE_HI: u8 = 0x7E;
    /// Identity BGP so color ID n → Shade n (bits 1–0 / 3–2 / 5–4 / 7–6).
    const BGP_IDENTITY: u8 = 0xE4;
    /// LCD on, BG on, unsigned `$8000` tiles, map `$9800` (same bits as post-boot `$91`).
    const LCDC_BG_8000: u8 = 0x91;
    /// Map tiles (2,2)–(3,3) → screen pixels [16, 32) × [16, 32) with SCX=SCY=0.
    const RECT_X0: usize = 16;
    const RECT_Y0: usize = 16;
    const RECT_W: usize = 16;
    const RECT_H: usize = 16;
    /// Two VBlanks after the overlay enables the LCD (LY reset on LCD-on).
    const TARGET_FRAMES: u64 = 2;

    /// 256-byte synthetic DMG boot overlay.
    ///
    /// ```text
    /// LD HL, $8010          ; tile 1
    /// LD B, 8
    /// loop: LD A, $3C / LD (HL+), A / LD A, $7E / LD (HL+), A / DEC B / JR NZ
    /// LD A, 1
    /// LD HL, $9842 / LD (HL+), A / LD (HL+), A   ; map (2,2) (3,2)
    /// LD HL, $9862 / LD (HL+), A / LD (HL+), A   ; map (2,3) (3,3)
    /// LD A, $E4 / LDH ($47), A                   ; BGP
    /// LD A, $91 / LDH ($40), A                   ; LCDC — enable last
    /// JR $
    /// ```
    fn synthetic_boot_rom() -> [u8; 256] {
        let mut rom = [0u8; 256];
        let prog: &[u8] = &[
            0x21,
            0x10,
            0x80, // LD HL, $8010
            0x06,
            0x08, // LD B, 8
            0x3E,
            TILE_LO, // LD A, $3C
            0x22,    // LD (HL+), A
            0x3E,
            TILE_HI, // LD A, $7E
            0x22,    // LD (HL+), A
            0x05,    // DEC B
            0x20,
            0xF7, // JR NZ, loop ($0005)
            0x3E,
            0x01, // LD A, 1
            0x21,
            0x42,
            0x98, // LD HL, $9842
            0x22, // LD (HL+), A
            0x22, // LD (HL+), A
            0x21,
            0x62,
            0x98, // LD HL, $9862
            0x22, // LD (HL+), A
            0x22, // LD (HL+), A
            0x3E,
            BGP_IDENTITY, // LD A, $E4
            0xE0,
            0x47, // LDH ($47), A
            0x3E,
            LCDC_BG_8000, // LD A, $91
            0xE0,
            0x40, // LDH ($40), A — LCD on after VRAM is filled
            0x18,
            0xFE, // JR $
        ];
        rom[..prog.len()].copy_from_slice(prog);
        rom
    }

    /// Golden 16×16 constructed from `decode_tile_row` + `shade_from_bgp`, not a capture.
    fn golden_shade_rectangle() -> [[Shade; RECT_W]; RECT_H] {
        let color_row = decode_tile_row(TILE_LO, TILE_HI);
        let shade_row: [Shade; 8] =
            std::array::from_fn(|i| shade_from_bgp(color_row[i], BGP_IDENTITY));
        let mut rect = [[Shade::Lightest; RECT_W]; RECT_H];
        for ty in 0..2 {
            for tx in 0..2 {
                for py in 0..8 {
                    for px in 0..8 {
                        rect[ty * 8 + py][tx * 8 + px] = shade_row[px];
                    }
                }
            }
        }
        rect
    }

    fn copy_rect(fb: &Framebuffer) -> [[Shade; RECT_W]; RECT_H] {
        let mut out = [[Shade::Lightest; RECT_W]; RECT_H];
        for (y, row) in out.iter_mut().enumerate() {
            for (x, px) in row.iter_mut().enumerate() {
                *px = fb.pixel(RECT_X0 + x, RECT_Y0 + y);
            }
        }
        out
    }

    #[test]
    fn synthetic_boot_renders_known_shade_rectangle() {
        let boot = synthetic_boot_rom();
        assert_eq!(boot.len(), 256);

        let mut bus = Bus::from_rom(rom_with_valid_header());
        bus.enable_boot_rom(&boot);
        let mut cpu = Cpu::new();
        prepare_boot(BootMode::BootRom, &mut cpu, &mut bus);
        assert_eq!(bus.read8(LCDC), crate::ppu::LCDC_POWER_ON);

        let mut session = ExecSession::new();
        match session.run_frames(&mut cpu, &mut bus, TARGET_FRAMES) {
            RunOutcome::FrameLimit { frames, .. } => assert_eq!(frames, TARGET_FRAMES),
            other => panic!("expected {TARGET_FRAMES} frames, got {other:?}"),
        }

        let fb = &bus.ppu.framebuffer;
        let got = copy_rect(fb);
        let want = golden_shade_rectangle();
        assert_eq!(
            got,
            want,
            "screen [{RECT_X0}..{})×[{RECT_Y0}..{}) must match tile-1 / BGP identity golden",
            RECT_X0 + RECT_W,
            RECT_Y0 + RECT_H
        );
        assert_eq!(
            fb.pixel(0, 0),
            Shade::Lightest,
            "tile 0 (empty) should fill the origin"
        );
    }

    /// Optional smoke: hash a documented logo band from real `dmg_boot.bin`.
    /// Not a substitute for `synthetic_boot_renders_known_shade_rectangle`.
    #[test]
    #[ignore = "10A.0 extra — needs dmg_boot.bin + carts/tetris.gb"]
    fn optional_dmg_boot_logo_region_hash() {
        let boot =
            super::logo_diagnostic::load_boot_rom().expect("target/release/boot/dmg_boot.bin");
        let cart = super::logo_diagnostic::load_tetris().expect("carts/tetris.gb");
        let mut bus = crate::bus::Bus::new(cart);
        bus.enable_boot_rom(&boot);
        let mut cpu = Cpu::new();
        prepare_boot(BootMode::BootRom, &mut cpu, &mut bus);
        let mut session = ExecSession::new();
        match session.run_frames(&mut cpu, &mut bus, 120) {
            RunOutcome::FrameLimit { .. } => {}
            other => panic!("dmg boot did not reach 120 frames: {other:?}"),
        }
        let fb = &bus.ppu.framebuffer;
        // Approximate Nintendo-logo band (not a whole-framebuffer hash).
        const X0: usize = 48;
        const X1: usize = 112;
        const Y0: usize = 32;
        const Y1: usize = 56;
        let mut hash = 2166136261u32;
        let mut nonzero = 0u32;
        for y in Y0..Y1 {
            for x in X0..X1 {
                let s = fb.pixel(x, y).index();
                hash ^= u32::from(s);
                hash = hash.wrapping_mul(16777619);
                if s != 0 {
                    nonzero += 1;
                }
            }
        }
        println!(
            "dmg_boot logo-region FNV-1a [{X0}..{X1})×[{Y0}..{Y1}) hash={hash:08X} nonzero={nonzero}"
        );
        assert!(
            nonzero > 0,
            "logo band should contain non-lightest pixels after boot LCD-on"
        );
    }
}
