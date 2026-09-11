//! Commercial ROM smoke matrix.
//!
//! Not an accuracy proof — different titles exercise different hardware
//! assumptions. Missing ROMs are skipped; present ones must boot, produce
//! frames, and avoid CPU faults / stuck HALT within the frame budget.
//! Unsupported mappers must fail the load with a clear error (not silent
//! ROM-only fallthrough).

use graycart::{Bus, Cartridge, Cpu, StepError, apply_fast, step};
use std::path::{Path, PathBuf};

struct SmokeRom {
    name: &'static str,
    /// Candidate paths under `carts/` (first existing wins).
    candidates: &'static [&'static str],
}

const ROMS: &[SmokeRom] = &[
    SmokeRom {
        name: "Tetris",
        candidates: &["tetris.gb"],
    },
    SmokeRom {
        name: "Final Fantasy Adventure",
        candidates: &["final-fantasy-adventure.gb"],
    },
    SmokeRom {
        name: "Kirby's Pinball Land",
        candidates: &["kirbys-pinball-land.gb"],
    },
    SmokeRom {
        name: "Pokemon Red",
        candidates: &[
            "pokemon_red-version__usa-eu.gb",
            "pokemon_red_version--usa-eu.gb",
            "pokemon_red.gb",
        ],
    },
    SmokeRom {
        name: "Pokemon Blue",
        candidates: &["pokemon_blue-version.gb", "pokemon_blue.gb"],
    },
    SmokeRom {
        name: "Pokemon Yellow",
        candidates: &["pokemon_yellow-version.gb", "pokemon_yellow.gb"],
    },
    SmokeRom {
        name: "Link's Awakening",
        candidates: &[
            "legend-of-zelda_links-awakening__usa-eu.gb",
            "legend_of_zelda_the_links_awakening--usa-eu.gb",
            "links_awakening.gb",
        ],
    },
    SmokeRom {
        name: "Super Mario Land 2",
        candidates: &[
            "super-mario-land-2_6-golden-coins__usa-eu.gb",
            "super_mario_land_2_6_golden_coins--usa-eu.gb",
            "super_mario_land_2.gb",
        ],
    },
    SmokeRom {
        name: "Wario Land",
        candidates: &[
            "wario-land_super-mario-land-3__world.gb",
            "wario_land_super_mario_land_3--usa-eu.gb",
            "wario_land.gb",
        ],
    },
];

#[derive(Debug, Clone)]
struct Checkpoint {
    frames: u32,
    ok: bool,
    note: String,
}

struct SmokeResult {
    name: &'static str,
    path: Option<PathBuf>,
    cp60: Option<Checkpoint>,
    cp300: Option<Checkpoint>,
    cp1000: Option<Checkpoint>,
    visual: &'static str,
}

fn resolve(rom: &SmokeRom) -> Option<PathBuf> {
    for name in rom.candidates {
        let p = Path::new("carts").join(name);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

fn run_until_frames(path: &Path, target_frames: u32) -> Result<Checkpoint, String> {
    let cart = Cartridge::load(path).map_err(|e| e.to_string())?;
    let mut cpu = Cpu::new();
    let mut bus = Bus::new(cart);
    apply_fast(&mut cpu, &mut bus);

    const STEPS_PER_FRAME_HALT: u32 = 70_224 / 4; // ~1 frame if always halted
    let max_steps = target_frames
        .saturating_mul(STEPS_PER_FRAME_HALT)
        .saturating_mul(4);
    let stuck_halt = STEPS_PER_FRAME_HALT.saturating_mul(3); // ~3 frames stuck

    let mut frames = 0u32;
    let mut halt_steps = 0u32;

    for _ in 0..max_steps {
        match step(&mut cpu, &mut bus) {
            Ok(_) => {
                if bus.ppu.take_frame_ready() {
                    frames += 1;
                    if frames >= target_frames {
                        return Ok(Checkpoint {
                            frames,
                            ok: true,
                            note: format!(
                                "PASS LCDC={:02X} LY={:02X} STAT={:02X} mode={} IF={:02X} IE={:02X} halted={} PC={:04X}",
                                bus.read8(0xFF40),
                                bus.read8(0xFF44),
                                bus.read8(0xFF41),
                                bus.read8(0xFF41) & 3,
                                bus.read8(0xFF0F),
                                bus.read8(0xFFFF),
                                cpu.halted,
                                cpu.pc,
                            ),
                        });
                    }
                }
                if cpu.halted {
                    halt_steps += 1;
                    if halt_steps >= stuck_halt {
                        return Ok(Checkpoint {
                            frames,
                            ok: false,
                            note: "stuck HALT".into(),
                        });
                    }
                } else {
                    halt_steps = 0;
                }
            }
            Err(StepError::Decode { .. }) => {
                return Ok(Checkpoint {
                    frames,
                    ok: false,
                    note: "CPU fault (decode)".into(),
                });
            }
            Err(StepError::Unimplemented { .. }) => {
                return Ok(Checkpoint {
                    frames,
                    ok: false,
                    note: "CPU fault (unimplemented)".into(),
                });
            }
        }
    }
    Ok(Checkpoint {
        frames,
        ok: false,
        note: format!(
            "frame budget LCDC={:02X} LY={:02X} STAT={:02X} mode={} IF={:02X} IE={:02X} halted={} PC={:04X} A={:02X}",
            bus.read8(0xFF40),
            bus.read8(0xFF44),
            bus.read8(0xFF41),
            bus.read8(0xFF41) & 3,
            bus.read8(0xFF0F),
            bus.read8(0xFFFF),
            cpu.halted,
            cpu.pc,
            cpu.a,
        ),
    })
}

fn fmt_cp(cp: Option<&Checkpoint>) -> String {
    match cp {
        None => "SKIP".into(),
        Some(c) if c.ok => format!("PASS({}f)", c.frames),
        Some(c) => format!("FAIL({}:{})", c.note, c.frames),
    }
}

#[test]
#[ignore = "commercial smoke matrix; needs ROMs under carts/"]
fn commercial_rom_smoke_matrix() {
    let mut results = Vec::new();
    let mut any_present = false;

    for rom in ROMS {
        let Some(path) = resolve(rom) else {
            results.push(SmokeResult {
                name: rom.name,
                path: None,
                cp60: None,
                cp300: None,
                cp1000: None,
                visual: "(ROM missing)",
            });
            continue;
        };
        any_present = true;

        let cp60 = match run_until_frames(&path, 60) {
            Ok(cp) => cp,
            Err(e) => Checkpoint {
                frames: 0,
                ok: false,
                note: e,
            },
        };
        let cp300 = match run_until_frames(&path, 300) {
            Ok(cp) => cp,
            Err(e) => Checkpoint {
                frames: 0,
                ok: false,
                note: e,
            },
        };
        let cp1000 = match run_until_frames(&path, 1000) {
            Ok(cp) => cp,
            Err(e) => Checkpoint {
                frames: 0,
                ok: false,
                note: e,
            },
        };

        results.push(SmokeResult {
            name: rom.name,
            path: Some(path),
            cp60: Some(cp60),
            cp300: Some(cp300),
            cp1000: Some(cp1000),
            visual: "see --run / manual",
        });
    }

    eprintln!();
    eprintln!(
        "{:<28} {:<14} {:<14} {:<14} visual",
        "ROM", "60f", "300f", "1000f"
    );
    for r in &results {
        eprintln!(
            "{:<28} {:<14} {:<14} {:<14} {}",
            r.name,
            fmt_cp(r.cp60.as_ref()),
            fmt_cp(r.cp300.as_ref()),
            fmt_cp(r.cp1000.as_ref()),
            r.visual
        );
        if let Some(p) = &r.path {
            eprintln!("  path: {}", p.display());
        }
        if let Some(cp) = r.cp60.as_ref() {
            eprintln!("  dump60: {}", cp.note);
        }
    }
    eprintln!();

    if !any_present {
        eprintln!("no commercial ROMs found under carts/; matrix skipped");
        return;
    }

    for r in &results {
        if r.path.is_none() {
            continue;
        }
        for (label, cp) in [("60f", &r.cp60), ("300f", &r.cp300), ("1000f", &r.cp1000)] {
            let cp = cp.as_ref().expect("present ROM always has checkpoint");
            assert!(
                cp.ok,
                "{} failed at {}: {} (frames={})",
                r.name, label, cp.note, cp.frames
            );
        }
    }
}

#[test]
#[ignore = "DMG 0-frame dump vs Tetris; 60f only"]
fn dmg_zero_frame_startup_dump() {
    let names = [
        "Tetris",
        "Final Fantasy Adventure",
        "Kirby's Pinball Land",
        "Link's Awakening",
    ];
    for rom in ROMS.iter().filter(|r| names.contains(&r.name)) {
        let Some(path) = resolve(rom) else {
            eprintln!("{} SKIP missing ROM", rom.name);
            continue;
        };
        match run_until_frames(&path, 60) {
            Ok(cp) => eprintln!(
                "{} {} frames={} {}",
                rom.name,
                if cp.ok { "PASS" } else { "FAIL" },
                cp.frames,
                cp.note
            ),
            Err(e) => eprintln!("{} load error {e}", rom.name),
        }
    }
}
