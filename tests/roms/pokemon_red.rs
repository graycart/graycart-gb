//! Commercial ROM ridge finder (Pokémon Red).

use graycart::{Bus, Cartridge, Cpu, apply_fast, step};
use std::collections::HashMap;
use std::path::Path;

#[test]
#[ignore = "ridge finder; needs carts/pokemon_red-version__usa-eu.gb"]
fn pokemon_red_runs_until_unknown_opcode() {
    let path = Path::new("carts/pokemon_red-version__usa-eu.gb");
    if !path.exists() {
        return;
    }
    let cart = Cartridge::load(path).unwrap();
    let mut cpu = Cpu::new();
    let mut bus = Bus::new(cart);
    apply_fast(&mut cpu, &mut bus);
    let mut hot: HashMap<u16, u32> = HashMap::new();
    // ~3 frames at 4 T-cycles per halted step (70224 T/frame).
    const HALT_STUCK_STEPS: u32 = 60_000;
    let mut halt_steps = 0u32;

    for i in 0..2_000_000u32 {
        match step(&mut cpu, &mut bus) {
            Ok(_) => {
                *hot.entry(cpu.pc).or_insert(0) += 1;
                if cpu.halted {
                    if halt_steps == 0 {
                        eprintln!(
                            "pokemon halt enter: steps={i} PC={:04X} bank={:02X} AF={:04X} BC={:04X} DE={:04X} HL={:04X} SP={:04X} IME={} IE={:02X} IF={:02X} LY={:02X} LCDC={:02X}",
                            cpu.pc,
                            bus.cartridge.switchable_rom_bank(),
                            cpu.af(),
                            cpu.bc(),
                            cpu.de(),
                            cpu.hl(),
                            cpu.sp,
                            cpu.ime,
                            bus.read8(0xFFFF),
                            bus.read8(0xFF0F),
                            bus.read8(0xFF44),
                            bus.read8(0xFF40),
                        );
                    }
                    halt_steps += 1;
                    if halt_steps >= HALT_STUCK_STEPS {
                        eprintln!(
                            "pokemon stuck halt: waited={halt_steps} PC={:04X} bank={:02X} IME={} IE={:02X} IF={:02X} LY={:02X} LCDC={:02X}",
                            cpu.pc,
                            bus.cartridge.switchable_rom_bank(),
                            cpu.ime,
                            bus.read8(0xFFFF),
                            bus.read8(0xFF0F),
                            bus.read8(0xFF44),
                            bus.read8(0xFF40),
                        );
                        return;
                    }
                } else if halt_steps > 0 {
                    eprintln!(
                        "pokemon halt wake: waited={halt_steps} steps PC={:04X} IF={:02X} LY={:02X}",
                        cpu.pc,
                        bus.read8(0xFF0F),
                        bus.read8(0xFF44),
                    );
                    halt_steps = 0;
                }
                if i > 0 && i % 200_000 == 0 {
                    let mut top: Vec<_> = hot.iter().collect();
                    top.sort_by_key(|(_, c)| std::cmp::Reverse(*c));
                    let summary: Vec<_> = top
                        .iter()
                        .take(5)
                        .map(|(pc, c)| format!("{pc:04X}:{c}"))
                        .collect();
                    let bytes: Vec<_> = (0..12)
                        .map(|o| format!("{:02X}", bus.read8(cpu.pc.wrapping_add(o))))
                        .collect();
                    eprintln!(
                        "pokemon @{i}: PC={:04X} bank={:02X} AF={:04X} DE={:04X} HL={:04X} LY={:02X} bytes={} hot=[{}]",
                        cpu.pc,
                        bus.cartridge.switchable_rom_bank(),
                        cpu.af(),
                        cpu.de(),
                        cpu.hl(),
                        bus.read8(0xFF44),
                        bytes.join(" "),
                        summary.join(" ")
                    );
                    hot.clear();
                }
            }
            Err(e) => {
                let bytes: Vec<_> = (0..8)
                    .map(|o| format!("{:02X}", bus.read8(cpu.pc.wrapping_add(o))))
                    .collect();
                eprintln!(
                    "pokemon stop: steps={i} PC={:04X} bank={:02X} SP={:04X} AF={:04X} BC={:04X} DE={:04X} HL={:04X}: {e}",
                    cpu.pc,
                    bus.cartridge.switchable_rom_bank(),
                    cpu.sp,
                    cpu.af(),
                    cpu.bc(),
                    cpu.de(),
                    cpu.hl(),
                );
                eprintln!("bytes @PC: {}", bytes.join(" "));
                return;
            }
        }
    }
    let mut top: Vec<_> = hot.iter().collect();
    top.sort_by_key(|(_, c)| std::cmp::Reverse(*c));
    let summary: Vec<_> = top
        .iter()
        .take(8)
        .map(|(pc, c)| format!("{pc:04X}:{c}"))
        .collect();
    eprintln!(
        "pokemon limit: PC={:04X} bank={:02X} AF={:04X} BC={:04X} DE={:04X} HL={:04X} SP={:04X} hot=[{}]",
        cpu.pc,
        bus.cartridge.switchable_rom_bank(),
        cpu.af(),
        cpu.bc(),
        cpu.de(),
        cpu.hl(),
        cpu.sp,
        summary.join(" "),
    );
}
