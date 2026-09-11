//! Forced Native CGB smoke (Fast / skip-boot) after 10I (`CGB_SILICON_READY`).
//!
//! Host pref is `GameBoyColor` so CGB-compatible carts stay Native even if
//! a future pref default changes. Automatic is the same matrix for these headers.
//!
//! Visual check: Native RGB555 produced **during** the run (`peak_nz` and
//! frames with any non-zero pixel). A single late frame may be a software
//! palette/tilemap clear (Crystal intro `Intro_ClearBGPals` / `ClearTilemap`
//! between jumptable scenes). That is not an all-zero LCD for the whole run.

use graycart::{
    Bus, CGB_SILICON_READY, Cartridge, CgbExecutionMode, Cpu, HardwareModel, HostHardwarePref,
    StepError, apply_fast, bus_from_cartridge, step,
};
use std::path::{Path, PathBuf};

struct Title {
    name: &'static str,
    path: &'static str,
}

const TITLES: &[Title] = &[
    Title {
        name: "Pokemon Crystal",
        path: "carts/pokemon_crystal-version.gbc",
    },
    Title {
        name: "Wario Land 3",
        path: "carts/wario-land-3.gbc",
    },
    Title {
        name: "Link's Awakening DX",
        path: "carts/legend-of-zelda_links-awakening_dx.gbc",
    },
];

const HDMA5: u16 = 0xFF55;
const KEY1: u16 = 0xFF4D;
const VBK: u16 = 0xFF4F;
const SVBK: u16 = 0xFF70;

fn shade_hist(bus: &Bus) -> [u32; 4] {
    let mut hist = [0u32; 4];
    for px in bus.ppu.framebuffer.pixels() {
        hist[px.index() as usize] += 1;
    }
    hist
}

fn rgb555_unique_and_nonzero(bus: &Bus) -> (usize, u32) {
    let mut uniq = std::collections::BTreeSet::new();
    let mut nonzero = 0u32;
    for &c in bus.ppu.framebuffer.rgb555_pixels() {
        let c = c & 0x7FFF;
        uniq.insert(c);
        if c != 0 {
            nonzero += 1;
        }
    }
    (uniq.len(), nonzero)
}

struct Live<'a> {
    title: &'a Title,
    path: &'a Path,
    model: HardwareModel,
    cgb_flag: String,
    a_at_entry: u8,
    frames: u32,
    banks: usize,
    hdma5_not_ff: u32,
    key1_double: bool,
}

fn fail_run(live: Live<'_>, bus: &Bus, note: String) -> Run {
    let (uniq, nz) = rgb555_unique_and_nonzero(bus);
    Run {
        name: live.title.name,
        path: live.path.to_path_buf(),
        model: live.model,
        cgb_flag: live.cgb_flag,
        a_at_entry: live.a_at_entry,
        frames: live.frames,
        unique_rom_banks: live.banks,
        hdma5_not_ff: live.hdma5_not_ff,
        key1_double: live.key1_double,
        last_hist: shade_hist(bus),
        unique_rgb555: uniq,
        rgb555_nonzero: nz,
        presents_cgb_color: bus.ppu.framebuffer.presents_cgb_color(),
        ok: false,
        note,
    }
}

fn ridge(cpu: &Cpu, bus: &Bus, frames: u32, why: &str) -> String {
    let bytes: Vec<_> = (0..12)
        .map(|o| format!("{:02X}", bus.read8(cpu.pc.wrapping_add(o))))
        .collect();
    format!(
        "{why} frames={frames} PC={:04X} bank={:02X} A={:02X} AF={:04X} BC={:04X} DE={:04X} HL={:04X} SP={:04X} IME={} halted={} IE={:02X} IF={:02X} LY={:02X} LCDC={:02X} KEY1={:02X} VBK={:02X} SVBK={:02X} HDMA5={:02X} bytes=[{}]",
        cpu.pc,
        bus.cartridge.switchable_rom_bank(),
        cpu.a,
        cpu.af(),
        cpu.bc(),
        cpu.de(),
        cpu.hl(),
        cpu.sp,
        cpu.ime,
        cpu.halted,
        bus.read8(0xFFFF),
        bus.read8(0xFF0F),
        bus.read8(0xFF44),
        bus.read8(0xFF40),
        bus.read8(KEY1),
        bus.read8(VBK),
        bus.read8(SVBK),
        bus.read8(HDMA5),
        bytes.join(" ")
    )
}

struct Run {
    name: &'static str,
    path: PathBuf,
    model: HardwareModel,
    cgb_flag: String,
    a_at_entry: u8,
    frames: u32,
    unique_rom_banks: usize,
    hdma5_not_ff: u32,
    key1_double: bool,
    last_hist: [u32; 4],
    unique_rgb555: usize,
    rgb555_nonzero: u32,
    presents_cgb_color: bool,
    ok: bool,
    note: String,
}

fn smoke_one(title: &Title) -> Option<Run> {
    let path = Path::new(title.path);
    if !path.exists() {
        eprintln!("SKIP {} (missing {})", title.name, title.path);
        return None;
    }

    let cart = Cartridge::load(path).expect("load cart");
    let cgb_flag = format!("{:?}", cart.header.cgb);
    const {
        assert!(
            CGB_SILICON_READY,
            "10I: Automatic CGB silicon is the product path"
        );
    }

    let mut cpu = Cpu::new();
    let mut bus =
        bus_from_cartridge(cart, HostHardwarePref::GameBoyColor).expect("force Game Boy Color");
    let model = bus.hardware_model();
    assert_eq!(
        model,
        HardwareModel::Cgb {
            mode: CgbExecutionMode::NativeCgb
        },
        "{} should launch NativeCgb when forced GBC",
        title.name
    );

    apply_fast(&mut cpu, &mut bus);

    const STEPS_PER_FRAME_HALT: u32 = 70_224 / 4;
    let target_frames = 1000u32;
    let max_steps = target_frames
        .saturating_mul(STEPS_PER_FRAME_HALT)
        .saturating_mul(4);
    let stuck_halt = STEPS_PER_FRAME_HALT.saturating_mul(3);

    let mut frames = 0u32;
    let mut halt_steps = 0u32;
    let mut banks = std::collections::BTreeSet::new();
    let mut hdma5_not_ff = 0u32;
    let mut key1_double = false;
    let mut peak_rgb555_nz = 0u32;
    let mut frames_with_rgb = 0u32;
    let a_at_entry = cpu.a;

    for _ in 0..max_steps {
        banks.insert(bus.cartridge.switchable_rom_bank());
        if bus.read8(HDMA5) != 0xFF {
            hdma5_not_ff += 1;
        }
        if bus.read8(KEY1) & 0x80 != 0 {
            key1_double = true;
        }

        match step(&mut cpu, &mut bus) {
            Ok(_) => {
                if bus.ppu.take_frame_ready() {
                    frames += 1;
                    let (uniq, nz) = rgb555_unique_and_nonzero(&bus);
                    peak_rgb555_nz = peak_rgb555_nz.max(nz);
                    if nz > 0 {
                        frames_with_rgb += 1;
                    }
                    if frames == 60 || frames == 300 || frames == 900 || frames == 1000 {
                        let hist = shade_hist(&bus);
                        eprintln!(
                            "{} @{frames}f hist={hist:?} rgb555_unique={uniq} rgb555_nz={nz} peak_nz={peak_rgb555_nz} color_frames={frames_with_rgb} color={} banks={} hdma_active_samples={hdma5_not_ff} double={key1_double} {}",
                            title.name,
                            bus.ppu.framebuffer.presents_cgb_color(),
                            banks.len(),
                            ridge(&cpu, &bus, frames, "checkpoint")
                        );
                    }
                    if frames >= target_frames {
                        let hist = shade_hist(&bus);
                        let color = bus.ppu.framebuffer.presents_cgb_color();
                        // Last-frame nz can be 0 during a scene-clear; require
                        // native color on many frames, not only frame 1000.
                        let visual_ok = color && peak_rgb555_nz > 0 && frames_with_rgb >= 100;
                        let unique_shades = hist.iter().filter(|&&n| n > 0).count();
                        let note = format!(
                            "alive unique_shades={unique_shades} last_hist={hist:?} last_rgb555_unique={uniq} last_nz={nz} peak_nz={peak_rgb555_nz} color_frames={frames_with_rgb} color={color} banks={} hdma_samples={hdma5_not_ff} double={key1_double}",
                            banks.len()
                        );
                        return Some(Run {
                            name: title.name,
                            path: path.to_path_buf(),
                            model,
                            cgb_flag,
                            a_at_entry,
                            frames,
                            unique_rom_banks: banks.len(),
                            hdma5_not_ff,
                            key1_double,
                            last_hist: hist,
                            unique_rgb555: uniq,
                            rgb555_nonzero: peak_rgb555_nz,
                            presents_cgb_color: color,
                            ok: visual_ok,
                            note,
                        });
                    }
                }
                if cpu.halted {
                    halt_steps += 1;
                    if halt_steps >= stuck_halt {
                        eprintln!("{}", ridge(&cpu, &bus, frames, "stuck HALT"));
                        return Some(fail_run(
                            Live {
                                title,
                                path,
                                model,
                                cgb_flag,
                                a_at_entry,
                                frames,
                                banks: banks.len(),
                                hdma5_not_ff,
                                key1_double,
                            },
                            &bus,
                            ridge(&cpu, &bus, frames, "stuck HALT"),
                        ));
                    }
                } else {
                    halt_steps = 0;
                }
            }
            Err(StepError::Decode { pc, bytes }) => {
                let why = format!(
                    "CPU fault (decode) opcode={:02X} pc={pc:04X} dump={bytes:02X?}",
                    bytes[0]
                );
                eprintln!("{}", ridge(&cpu, &bus, frames, &why));
                return Some(fail_run(
                    Live {
                        title,
                        path,
                        model,
                        cgb_flag,
                        a_at_entry,
                        frames,
                        banks: banks.len(),
                        hdma5_not_ff,
                        key1_double,
                    },
                    &bus,
                    ridge(&cpu, &bus, frames, &why),
                ));
            }
            Err(StepError::Unimplemented { pc, instruction }) => {
                let why = format!("CPU fault (unimplemented) {instruction:?} pc={pc:04X}");
                eprintln!("{}", ridge(&cpu, &bus, frames, &why));
                return Some(fail_run(
                    Live {
                        title,
                        path,
                        model,
                        cgb_flag,
                        a_at_entry,
                        frames,
                        banks: banks.len(),
                        hdma5_not_ff,
                        key1_double,
                    },
                    &bus,
                    ridge(&cpu, &bus, frames, &why),
                ));
            }
        }
    }

    eprintln!("{}", ridge(&cpu, &bus, frames, "frame budget"));
    Some(fail_run(
        Live {
            title,
            path,
            model,
            cgb_flag,
            a_at_entry,
            frames,
            banks: banks.len(),
            hdma5_not_ff,
            key1_double,
        },
        &bus,
        ridge(&cpu, &bus, frames, "frame budget"),
    ))
}

#[test]
#[ignore = "forced Native CGB smoke; needs CGB carts; Fast skip-boot"]
fn native_cgb_forced_game_boy_color_smoke() {
    const {
        assert!(CGB_SILICON_READY);
    }
    let mut runs = Vec::new();
    for title in TITLES {
        if let Some(run) = smoke_one(title) {
            runs.push(run);
        }
    }

    eprintln!();
    eprintln!(
        "CGB_SILICON_READY={} (Automatic uses CGB silicon)",
        CGB_SILICON_READY
    );
    eprintln!(
        "{:<24} {:<16} {:<8} {:<8} {:<10} {:<8} hist",
        "title", "flag", "frames", "banks", "hdma", "double"
    );
    for r in &runs {
        eprintln!(
            "{:<24} {:<16} {:<8} {:<8} {:<10} {} {:?}",
            r.name,
            r.cgb_flag,
            r.frames,
            r.unique_rom_banks,
            r.hdma5_not_ff,
            r.key1_double,
            r.last_hist
        );
        eprintln!(
            "  path={} model={:?} A@0100={:02X} rgb555_unique={} nz={} color={} {}",
            r.path.display(),
            r.model,
            r.a_at_entry,
            r.unique_rgb555,
            r.rgb555_nonzero,
            r.presents_cgb_color,
            r.note
        );
        if !r.ok {
            eprintln!("  ridge: {}", r.note);
        }
    }
    eprintln!(
        "Fast NativeCgb uses after_boot_cgb (A=$11). Peak RGB555 must not be an all-zero run."
    );
    eprintln!();

    assert!(!runs.is_empty(), "need at least one CGB ROM under carts/");
    for r in &runs {
        assert!(r.ok, "{} smoke failed: {}", r.name, r.note);
    }
}
