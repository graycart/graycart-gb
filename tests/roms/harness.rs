//! Shared conformance ROM runner (Blargg / Mooneye).
//!
//! Fixtures declare [`RomLaunchMode`] so a power-on ROM is never scored as Fast
//! post-boot, and vice versa.

use graycart::{
    Bus, Cartridge, Cpu, HostHardwarePref, StepError, apply_fast, bus_from_cartridge, step,
};
use std::path::Path;

/// How a ROM fixture is launched. Matches the hardware condition the ROM inspects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RomLaunchMode {
    /// `Cpu::new` + CGB silicon from `GameBoyColor`. No `apply_fast`. LCDC power-on.
    PowerOn,
    /// DMG silicon + [`apply_fast`].
    FastDmg,
    /// `bus_from_cartridge(GameBoyColor)` + [`apply_fast`].
    FastCgb,
    /// Power-on CPU with boot firmware mapped. Missing firmware → [`Outcome::Unsupported`].
    BootRom,
}

/// Result of one conformance ROM run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Pass,
    Fail(String),
    Timeout,
    Unsupported(String),
}

impl Outcome {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Fail(_) => "FAIL",
            Self::Timeout => "TIMEOUT",
            Self::Unsupported(_) => "UNSUPPORTED",
        }
    }

    pub fn detail(&self) -> &str {
        match self {
            Self::Pass | Self::Timeout => "",
            Self::Fail(s) | Self::Unsupported(s) => s,
        }
    }
}

/// Deterministic DMG Fast path for Blargg / Mooneye DMG acceptance.
pub fn load_machine(path: &Path) -> Result<(Cpu, Bus), String> {
    load_machine_launched(path, RomLaunchMode::FastDmg)
}

/// Same Fast handoff after [`bus_from_cartridge`] (CGB smoke / CGB Mooneye).
pub fn load_machine_with_pref(path: &Path, pref: HostHardwarePref) -> Result<(Cpu, Bus), String> {
    let mode = match pref {
        HostHardwarePref::GameBoy => RomLaunchMode::FastDmg,
        HostHardwarePref::GameBoyColor | HostHardwarePref::Automatic => RomLaunchMode::FastCgb,
    };
    load_machine_launched(path, mode)
}

/// Launch a fixture under the machine state it is designed to inspect.
pub fn load_machine_launched(path: &Path, mode: RomLaunchMode) -> Result<(Cpu, Bus), String> {
    let cart = Cartridge::load(path).map_err(|e| e.to_string())?;
    match mode {
        RomLaunchMode::FastDmg => {
            let mut cpu = Cpu::new();
            let mut bus = Bus::new(cart);
            apply_fast(&mut cpu, &mut bus);
            Ok((cpu, bus))
        }
        RomLaunchMode::FastCgb => {
            let mut cpu = Cpu::new();
            let mut bus = bus_from_cartridge(cart, HostHardwarePref::GameBoyColor)
                .map_err(|e| e.to_string())?;
            apply_fast(&mut cpu, &mut bus);
            Ok((cpu, bus))
        }
        RomLaunchMode::PowerOn => {
            let cpu = Cpu::new();
            let bus = bus_from_cartridge(cart, HostHardwarePref::GameBoyColor)
                .map_err(|e| e.to_string())?;
            Ok((cpu, bus))
        }
        RomLaunchMode::BootRom => Err(
            "BootRom launch requires mapped firmware; use PowerOn or Fast until a fixture provides it"
                .into(),
        ),
    }
}

/// Blargg-style: watch serial `$FF01` **or** cart-RAM `$A000` result.
///
/// Newer shells (e.g. `dmg_sound`) write status to `$A000` with signature
/// `$DE $B0 $61` at `$A001–$A003` and a C string at `$A004` (see Blargg readme).
pub fn run_blargg(path: &Path, step_limit: u32) -> Outcome {
    run_blargg_launched(path, step_limit, RomLaunchMode::FastDmg)
}

/// Blargg runner with an explicit launch mode (`cgb_sound` is Native CGB).
pub fn run_blargg_launched(path: &Path, step_limit: u32, mode: RomLaunchMode) -> Outcome {
    let (mut cpu, mut bus) = match load_machine_launched(path, mode) {
        Ok(m) => m,
        Err(e) => return Outcome::Unsupported(e),
    };

    for i in 0..step_limit {
        match step(&mut cpu, &mut bus) {
            Ok(_) => {
                if let Some(o) = blargg_result(&bus) {
                    return o;
                }
            }
            Err(e) => return unsupported_step(i, &cpu, &bus, e),
        }
    }
    blargg_result(&bus).unwrap_or_else(|| {
        let s = summarize_serial(&bus.serial_text());
        let ram = blargg_ram_text(&bus);
        if s.is_empty() && ram.is_empty() {
            Outcome::Timeout
        } else {
            Outcome::Fail(format!("no pass/fail yet; serial={s} ram={ram}"))
        }
    })
}

fn blargg_result(bus: &Bus) -> Option<Outcome> {
    let text = bus.serial_text();
    if text.contains("Passed") {
        return Some(Outcome::Pass);
    }
    if text.contains("Failed") {
        return Some(Outcome::Fail(summarize_serial(&text)));
    }

    // Cart-RAM oracle (`dmg_sound`, etc.).
    if bus.read8(0xA001) == 0xDE && bus.read8(0xA002) == 0xB0 && bus.read8(0xA003) == 0x61 {
        let status = bus.read8(0xA000);
        if status == 0x80 {
            return None; // still running
        }
        let ram_text = blargg_ram_text(bus);
        if status == 0 {
            return Some(Outcome::Pass);
        }
        let detail = if ram_text.is_empty() {
            format!("code={status}")
        } else {
            format!("code={status} {ram_text}")
        };
        return Some(Outcome::Fail(detail));
    }
    None
}

fn blargg_ram_text(bus: &Bus) -> String {
    let mut bytes = Vec::new();
    for addr in 0xA004u16..=0xA0FF {
        let b = bus.read8(addr);
        if b == 0 {
            break;
        }
        bytes.push(b);
    }
    let s = String::from_utf8_lossy(&bytes);
    summarize_serial(&s)
}

/// Mooneye-style: Fibonacci register success / infinite-loop failure.
pub fn run_mooneye(path: &Path, step_limit: u32) -> Outcome {
    run_mooneye_loaded(load_machine(path), step_limit)
}

/// Mooneye runner after [`bus_from_cartridge`] (CGB `-C` tests).
pub fn run_mooneye_with_pref(path: &Path, step_limit: u32, pref: HostHardwarePref) -> Outcome {
    run_mooneye_loaded(load_machine_with_pref(path, pref), step_limit)
}

pub fn run_mooneye_launched(path: &Path, step_limit: u32, mode: RomLaunchMode) -> Outcome {
    run_mooneye_loaded(load_machine_launched(path, mode), step_limit)
}

fn run_mooneye_loaded(loaded: Result<(Cpu, Bus), String>, step_limit: u32) -> Outcome {
    let (mut cpu, mut bus) = match loaded {
        Ok(m) => m,
        Err(e) => return Outcome::Unsupported(e),
    };

    let mut same_pc = 0u32;
    let mut last_pc = cpu.pc;

    for i in 0..step_limit {
        match step(&mut cpu, &mut bus) {
            Ok(_) => {
                if mooneye_success(&cpu) {
                    return Outcome::Pass;
                }
                if cpu.pc == last_pc {
                    same_pc = same_pc.saturating_add(1);
                } else {
                    same_pc = 0;
                    last_pc = cpu.pc;
                }
                // Tight infinite loop that is not the success fibonacci → fail.
                // Ignore HALT (PC frozen while waiting for an interrupt).
                if !cpu.halted && same_pc >= 64 && !mooneye_success(&cpu) {
                    return Outcome::Fail(format!(
                        "loop PC=${:04X} BC={:02X}{:02X} DE={:02X}{:02X} HL={:02X}{:02X}",
                        cpu.pc, cpu.b, cpu.c, cpu.d, cpu.e, cpu.h, cpu.l
                    ));
                }
                let text = bus.serial_text();
                if text.contains("Passed") {
                    return Outcome::Pass;
                }
                if text.contains("Failed") {
                    return Outcome::Fail(summarize_serial(&text));
                }
            }
            Err(e) => return unsupported_step(i, &cpu, &bus, e),
        }
    }
    if mooneye_success(&cpu) {
        Outcome::Pass
    } else {
        Outcome::Timeout
    }
}

fn mooneye_success(cpu: &Cpu) -> bool {
    cpu.b == 3 && cpu.c == 5 && cpu.d == 8 && cpu.e == 13 && cpu.h == 21 && cpu.l == 34
}

fn unsupported_step(step_i: u32, cpu: &Cpu, bus: &Bus, err: StepError) -> Outcome {
    Outcome::Unsupported(format!(
        "step={step_i} PC=${:04X} bank={:02X}: {err}",
        cpu.pc,
        bus.cartridge.switchable_rom_bank()
    ))
}

fn summarize_serial(text: &str) -> String {
    let flat: String = text
        .chars()
        .map(|c| if c == '\n' { ' ' } else { c })
        .collect();
    let flat = flat.trim();
    if flat.len() > 120 {
        format!("{}…", &flat[..120])
    } else {
        flat.to_string()
    }
}

#[cfg(test)]
mod launch_contract_tests {
    use super::*;
    const LCDC: u16 = 0xFF40;
    const LCDC_POWER_ON: u8 = 0x00;
    const LCDC_AFTER_BOOT: u8 = 0x91;

    fn tiny_rom() -> std::path::PathBuf {
        Path::new("tests/fixtures/mooneye/misc/boot_regs-cgb.gb").to_path_buf()
    }

    #[test]
    fn power_on_does_not_apply_fast() {
        let path = tiny_rom();
        if !path.exists() {
            return;
        }
        let (cpu, bus) = load_machine_launched(&path, RomLaunchMode::PowerOn).unwrap();
        assert_eq!(cpu.pc, 0);
        assert_eq!(bus.read8(LCDC), LCDC_POWER_ON);
        assert_ne!(bus.read8(LCDC), LCDC_AFTER_BOOT);
    }

    #[test]
    fn fast_cgb_is_post_boot() {
        let path = tiny_rom();
        if !path.exists() {
            return;
        }
        let (cpu, bus) = load_machine_launched(&path, RomLaunchMode::FastCgb).unwrap();
        assert_eq!(cpu.pc, 0x0100);
        assert_eq!(cpu.a, 0x11);
        assert_eq!(bus.read8(LCDC), LCDC_AFTER_BOOT);
        let (cpu2, _) = load_machine_with_pref(&path, HostHardwarePref::GameBoyColor).unwrap();
        assert_eq!(cpu2.a, cpu.a);
    }

    #[test]
    fn fast_dmg_is_post_boot_a_01() {
        let path = Path::new("tests/fixtures/mooneye/acceptance/bits/reg_f.gb");
        if !path.exists() {
            return;
        }
        let (cpu, bus) = load_machine_launched(path, RomLaunchMode::FastDmg).unwrap();
        assert_eq!(cpu.pc, 0x0100);
        assert_eq!(cpu.a, 0x01);
        assert_eq!(bus.read8(LCDC), LCDC_AFTER_BOOT);
    }

    #[test]
    fn boot_rom_without_firmware_is_unsupported() {
        let path = tiny_rom();
        if !path.exists() {
            return;
        }
        match load_machine_launched(&path, RomLaunchMode::BootRom) {
            Err(err) => assert!(err.contains("firmware")),
            Ok(_) => panic!("BootRom without firmware must fail"),
        }
        let _ = run_mooneye_with_pref(&path, 8, HostHardwarePref::GameBoyColor);
    }
}
