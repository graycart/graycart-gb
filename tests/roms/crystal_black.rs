//! Crystal intro diagnostics (ignored). Not a hardware patch surface.
//!
//! Authoritative: Pan Docs CGB palettes / HDMA / Crystal intro jumptable.
//! Frame 1000 all-zero RGB can be `Intro_ClearBGPals` / `ClearTilemap` between
//! scenes — use peak occupancy, not a single terminal frame.

use graycart::{
    Bus, Cartridge, Cpu, HostHardwarePref, StepError, apply_fast, bus_from_cartridge, step,
};
use std::collections::BTreeSet;
use std::path::Path;

const CART: &str = "carts/pokemon_crystal-version.gbc";
const HDMA1: u16 = 0xFF51;
const HDMA5: u16 = 0xFF55;

fn rgb555_unique_nz(bus: &Bus) -> (usize, u32) {
    let mut uniq = BTreeSet::new();
    let mut nz = 0u32;
    for &c in bus.ppu.framebuffer.rgb555_pixels() {
        let c = c & 0x7FFF;
        uniq.insert(c);
        if c != 0 {
            nz += 1;
        }
    }
    (uniq.len(), nz)
}

fn cram_unique(bus: &Bus) -> (usize, u32) {
    let mut uniq = BTreeSet::new();
    let mut nz = 0u32;
    for pal in 0..8u8 {
        for color in 0..4u8 {
            for rgb in [
                bus.ppu.cram.rgb555_bg(pal, color) & 0x7FFF,
                bus.ppu.cram.rgb555_obj(pal, color) & 0x7FFF,
            ] {
                uniq.insert(rgb);
                if rgb != 0 {
                    nz += 1;
                }
            }
        }
    }
    (uniq.len(), nz)
}

fn fnv32(data: &[u8]) -> u32 {
    let mut h = 0x811c_9dc5u32;
    for &b in data {
        h ^= u32::from(b);
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

fn hex16(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn dump_oam(bus: &Bus, label: &str) {
    let bytes = bus.ppu.oam.bytes();
    eprintln!("=== OAM {label} ===");
    let height = if bus.read8(0xFF40) & 0x04 != 0 { 16 } else { 8 };
    let mut hits = 0u32;
    for i in 0..40 {
        let y = bytes[i * 4];
        let x = bytes[i * 4 + 1];
        let tile = bytes[i * 4 + 2];
        let attr = bytes[i * 4 + 3];
        if y | x | tile | attr == 0 {
            continue;
        }
        eprintln!(
            "  [{i:02}] Y={y:02X} X={x:02X} tile={tile:02X} attr={attr:02X} screen=({},{})",
            x as i16 - 8,
            y as i16 - 16
        );
        let y16 = u16::from(y);
        for ly in 0..144u8 {
            let ly16 = u16::from(ly).wrapping_add(16);
            if ly16 >= y16 && ly16 < y16 + height as u16 {
                hits += 1;
            }
        }
    }
    eprintln!("  nonzero entries above; scanline hits={hits} height={height}");
}

fn dump_scanline_objs(bus: &Bus, ly: u8) {
    let bytes = bus.ppu.oam.bytes();
    let height = if bus.read8(0xFF40) & 0x04 != 0 { 16 } else { 8 };
    eprintln!("=== OBJ candidates LY={ly} height={height} ===");
    let mut n = 0u8;
    for i in 0..40 {
        let y = bytes[i * 4];
        let x = bytes[i * 4 + 1];
        let tile = bytes[i * 4 + 2];
        let attr = bytes[i * 4 + 3];
        let y16 = u16::from(y);
        let ly16 = u16::from(ly).wrapping_add(16);
        if ly16 >= y16 && ly16 < y16 + height as u16 {
            n += 1;
            eprintln!(
                "  #{n} [{i:02}] Y={y:02X} X={x:02X} tile={tile:02X} attr={attr:02X} (CGB pal={}, prio={}, bank={}, xflip={}, yflip={})",
                attr & 7,
                (attr >> 7) & 1,
                (attr >> 3) & 1,
                (attr >> 5) & 1,
                (attr >> 6) & 1,
            );
        }
    }
    if n == 0 {
        eprintln!("  (none)");
    }
}

fn cram_fingerprint(bus: &Bus) -> [u8; 128] {
    let mut out = [0u8; 128];
    for pal in 0..8u8 {
        for color in 0..4u8 {
            let i = (pal as usize) * 8 + (color as usize) * 2;
            let bg = bus.ppu.cram.rgb555_bg(pal, color);
            let obj = bus.ppu.cram.rgb555_obj(pal, color);
            out[i] = bg as u8;
            out[i + 1] = (bg >> 8) as u8;
            out[64 + i] = obj as u8;
            out[64 + i + 1] = (obj >> 8) as u8;
        }
    }
    out
}

fn snapshot(bus: &Bus, cpu: &Cpu, frames: u32) -> String {
    let (fb_u, fb_nz) = rgb555_unique_nz(bus);
    let (cram_u, cram_nz) = cram_unique(bus);
    let v0 = fnv32(bus.ppu.vram.bank(0).as_slice());
    let v1 = fnv32(bus.ppu.vram.bank(1).as_slice());
    let oam = fnv32(bus.ppu.oam.bytes().as_slice());
    let map0 = &bus.ppu.vram.bank(0)[0x1800..0x1810];
    let attr1 = &bus.ppu.vram.bank(1)[0x1800..0x1810];
    format!(
        "frame {frames}\n\
LCDC={:02X} STAT={:02X} LY={:02X} mode={} LCDCon={}\n\
SCY={:02X} SCX={:02X} WY={:02X} WX={:02X}\n\
VBK={:02X} SVBK={:02X} BGPI={:02X} OBPI={:02X} KEY1={:02X}\n\
HDMA1-5={:02X}{:02X}{:02X}{:02X}{:02X} bank={:02X} PC={:04X}\n\
CRAM unique={cram_u} nz={cram_nz} BG0={:04X}\n\
VRAM0 hash={v0:08X} VRAM1 hash={v1:08X} OAM hash={oam:08X}\n\
map9800={}\n\
attr9800={}\n\
framebuffer unique={fb_u} nz={fb_nz} color={}",
        bus.read8(0xFF40),
        bus.read8(0xFF41),
        bus.read8(0xFF44),
        bus.read8(0xFF41) & 0x03,
        bus.read8(0xFF40) & 0x80 != 0,
        bus.read8(0xFF42),
        bus.read8(0xFF43),
        bus.read8(0xFF4A),
        bus.read8(0xFF4B),
        bus.read8(0xFF4F),
        bus.read8(0xFF70),
        bus.read8(0xFF68),
        bus.read8(0xFF6A),
        bus.read8(0xFF4D),
        bus.read8(HDMA1),
        bus.read8(HDMA1 + 1),
        bus.read8(HDMA1 + 2),
        bus.read8(HDMA1 + 3),
        bus.read8(HDMA5),
        bus.cartridge.switchable_rom_bank(),
        cpu.pc,
        bus.ppu.cram.rgb555_bg(0, 0) & 0x7FFF,
        hex16(map0),
        hex16(attr1),
        bus.ppu.framebuffer.presents_cgb_color(),
    )
}

#[test]
#[ignore = "Crystal first-black-frame classification; release recommended"]
fn crystal_first_black_frame_classification() {
    let path = Path::new(CART);
    if !path.exists() {
        eprintln!("SKIP missing {CART}");
        return;
    }

    let cart = Cartridge::load(path).expect("load Crystal");
    let mut cpu = Cpu::new();
    let mut bus = bus_from_cartridge(cart, HostHardwarePref::GameBoyColor).expect("force GBC");
    apply_fast(&mut cpu, &mut bus);

    let mut frames = 0u32;
    let mut prev: Option<String> = None;
    let mut prev_oam: Option<[u8; 160]> = None;
    let mut cram_wipe: Option<(u32, String, String)> = None;
    let mut lcd_black: Option<(u32, String, String)> = None;
    let mut cram_prev = cram_fingerprint(&bus);
    let max_steps = 1000u32.saturating_mul(70_224 / 4).saturating_mul(4);

    for _ in 0..max_steps {
        match step(&mut cpu, &mut bus) {
            Ok(_) => {
                if (608..610).contains(&frames) {
                    let now = cram_fingerprint(&bus);
                    if now != cram_prev {
                        let mut diffs = Vec::new();
                        for i in 0..128 {
                            if now[i] != cram_prev[i] {
                                diffs
                                    .push(format!("{:02X}:{:02X}→{:02X}", i, cram_prev[i], now[i]));
                            }
                        }
                        eprintln!(
                            "CRAM change during frame {frames} PC={:04X} bank={:02X} BGPI={:02X} OBPI={:02X} STAT={:02X} unique={:?} bytes=[{}]",
                            cpu.pc,
                            bus.cartridge.switchable_rom_bank(),
                            bus.read8(0xFF68),
                            bus.read8(0xFF6A),
                            bus.read8(0xFF41),
                            cram_unique(&bus),
                            diffs.join(" ")
                        );
                        cram_prev = now;
                    }
                }
                if bus.ppu.take_frame_ready() {
                    frames += 1;
                    let snap = snapshot(&bus, &cpu, frames);
                    if frames == 593 || frames == 594 {
                        dump_oam(&bus, &format!("frame {frames}"));
                        dump_scanline_objs(&bus, 80);
                        if frames == 594
                            && let Some(old) = prev_oam
                        {
                            let new = *bus.ppu.oam.bytes();
                            eprintln!("=== OAM byte diffs 593→594 ===");
                            let mut n = 0u32;
                            for i in 0..160 {
                                if old[i] != new[i] {
                                    n += 1;
                                    eprintln!("  +{i:02X}: {:02X} → {:02X}", old[i], new[i]);
                                }
                            }
                            eprintln!("  {n} bytes differ");
                        }
                    }
                    prev_oam = Some(*bus.ppu.oam.bytes());
                    if frames == 608 {
                        cram_prev = cram_fingerprint(&bus);
                    }
                    if frames == 60 || frames == 300 || frames == 1000 {
                        eprintln!("{snap}\n");
                    }
                    let (fb_u, fb_nz) = rgb555_unique_nz(&bus);
                    let _ = fb_u;
                    if lcd_black.is_none() && frames > 300 && fb_nz == 0 {
                        let before = prev.clone().unwrap_or_else(|| "(no previous)".into());
                        lcd_black = Some((frames, before, snap.clone()));
                    }
                    if cram_wipe.is_none() && frames > 60 && cram_unique(&bus) == (1, 0) {
                        let before = prev.clone().unwrap_or_else(|| "(no previous)".into());
                        cram_wipe = Some((frames, before, snap.clone()));
                    }
                    prev = Some(snap);
                    if frames >= 1000 {
                        break;
                    }
                }
            }
            Err(StepError::Decode { pc, bytes }) => {
                panic!("decode fault pc={pc:04X} {bytes:02X?}");
            }
            Err(StepError::Unimplemented { pc, instruction }) => {
                panic!("unimplemented {instruction:?} pc={pc:04X}");
            }
        }
    }

    if let Some((n, before, after)) = lcd_black {
        eprintln!("=== first framebuffer all-zero after 300f = {n} ===\n");
        eprintln!("--- N-1 ---\n{before}\n");
        eprintln!("--- N ---\n{after}\n");
    }
    match cram_wipe {
        Some((n, before, after)) => {
            eprintln!("=== first CRAM wipe (unique=1 nz=0) frame = {n} ===\n");
            eprintln!("--- N-1 ---\n{before}\n");
            eprintln!("--- N ---\n{after}\n");
        }
        None => {
            eprintln!("no CRAM wipe through {frames} frames");
        }
    }
}

fn oam_all_zero(bus: &Bus) -> bool {
    bus.ppu.oam.bytes().iter().all(|&b| b == 0)
}

/// Events before the frame-594 black screen — diagnosis, not a hardware patch.
#[test]
#[ignore = "Crystal earliest visible events before frame 594"]
fn crystal_earliest_visible_before_594() {
    let path = Path::new(CART);
    if !path.exists() {
        eprintln!("SKIP missing {CART}");
        return;
    }

    let cart = Cartridge::load(path).expect("load Crystal");
    let mut cpu = Cpu::new();
    let mut bus = bus_from_cartridge(cart, HostHardwarePref::GameBoyColor).expect("force GBC");
    apply_fast(&mut cpu, &mut bus);
    assert!(
        bus.hardware_model().native_cgb(),
        "Crystal is CGB-only; probe NativeCgb, not compat MMIO"
    );

    let mut frames = 0u32;
    let mut dma_was = false;
    let mut dma_starts = 0u32;
    let mut first_oam_zero: Option<(u32, u16, u16, u8)> = None;
    let mut saw_oam_nz = false;
    let mut first_fb_zero: Option<(u32, u16, u16)> = None;
    let mut hdma5_prev = bus.read8(HDMA5);
    let mut cram0 = cram_fingerprint(&bus);
    let mut first_cram_change: Option<(u32, u16, u16)> = None;
    let mut key1_seen: Option<(u32, u16, u16)> = None;
    let mut vblank_hits = 0u32;
    let mut pal_flag_on_vblank = 0u32;
    let mut first_real_map: Option<(u32, u8)> = None;
    let max_steps = 700u32.saturating_mul(70_224 / 4).saturating_mul(4);

    for _ in 0..max_steps {
        let dma_now = bus.ppu.dma_active();
        if dma_now && !dma_was {
            dma_starts += 1;
            if dma_starts <= 24 {
                eprintln!(
                    "OAM DMA start #{dma_starts} frame={frames} LY={:02X} PC={:04X} bank={:02X} page={:02X} KEY1={:02X} HDMA5={:02X}",
                    bus.read8(0xFF44),
                    cpu.pc,
                    bus.cartridge.switchable_rom_bank(),
                    bus.ppu.dma_source_page(),
                    bus.read8(0xFF4D),
                    bus.read8(HDMA5),
                );
            }
        }
        dma_was = dma_now;

        let h5 = bus.read8(HDMA5);
        if h5 != hdma5_prev {
            if frames < 594 {
                eprintln!(
                    "HDMA5 {:02X}→{:02X} frame={frames} LY={:02X} PC={:04X} bank={:02X}",
                    hdma5_prev,
                    h5,
                    bus.read8(0xFF44),
                    cpu.pc,
                    bus.cartridge.switchable_rom_bank(),
                );
            }
            hdma5_prev = h5;
        }

        if key1_seen.is_none() && bus.read8(0xFF4D) & 0x80 != 0 {
            key1_seen = Some((frames, cpu.pc, bus.cartridge.switchable_rom_bank()));
            eprintln!(
                "KEY1 double-speed bit set frame={frames} PC={:04X} bank={:02X}",
                cpu.pc,
                bus.cartridge.switchable_rom_bank()
            );
        }

        if cpu.pc == 0x0040 {
            if bus.read8(0xFFE5) != 0 {
                pal_flag_on_vblank += 1;
            }
            if vblank_hits < 24 {
                vblank_hits += 1;
                eprintln!(
                    "VBlank $0040 #{vblank_hits} frame={frames} LY={:02X} STAT={:02X} hCGBPal={:02X} hCGB={:02X} FFE0-EF={}",
                    bus.read8(0xFF44),
                    bus.read8(0xFF41),
                    bus.read8(0xFFE5),
                    bus.read8(0xFFE6),
                    (0xFFE0u16..=0xFFEF)
                        .map(|a| format!("{:02X}", bus.read8(a)))
                        .collect::<Vec<_>>()
                        .join(" "),
                );
            }
        }

        match step(&mut cpu, &mut bus) {
            Ok(_) => {
                if !oam_all_zero(&bus) {
                    saw_oam_nz = true;
                }
                if first_oam_zero.is_none() && saw_oam_nz && oam_all_zero(&bus) {
                    first_oam_zero = Some((
                        frames,
                        cpu.pc,
                        bus.cartridge.switchable_rom_bank(),
                        bus.ppu.dma_source_page(),
                    ));
                    eprintln!(
                        "OAM all-zero first seen frame={frames} PC={:04X} bank={:02X} dma_page={:02X} dma_active={}",
                        cpu.pc,
                        bus.cartridge.switchable_rom_bank(),
                        bus.ppu.dma_source_page(),
                        bus.ppu.dma_active(),
                    );
                }
                if bus.ppu.take_frame_ready() {
                    frames += 1;
                    let cram = cram_fingerprint(&bus);
                    if first_cram_change.is_none() && cram != cram0 {
                        first_cram_change =
                            Some((frames, cpu.pc, bus.cartridge.switchable_rom_bank()));
                        eprintln!(
                            "first CRAM change vs Fast fill at frame {frames} PC={:04X} bank={:02X}",
                            cpu.pc,
                            bus.cartridge.switchable_rom_bank()
                        );
                        cram0 = cram;
                    }
                    let (_, fb_nz) = rgb555_unique_nz(&bus);
                    if first_fb_zero.is_none() && frames > 1 && fb_nz == 0 {
                        first_fb_zero = Some((frames, cpu.pc, bus.cartridge.switchable_rom_bank()));
                        eprintln!(
                            "first all-zero framebuffer frame={frames} PC={:04X} bank={:02X} LCDC={:02X}",
                            cpu.pc,
                            bus.cartridge.switchable_rom_bank(),
                            bus.read8(0xFF40)
                        );
                    }
                    if first_real_map.is_none() {
                        let map = &bus.ppu.vram.bank(0)[0x1800..0x1A40];
                        if let Some((i, t)) = map
                            .iter()
                            .enumerate()
                            .find(|(_, t)| **t != 0 && **t != 0x7F)
                        {
                            first_real_map = Some((frames, *t));
                            eprintln!(
                                "first non-blank BG tile at frame {frames} map+{i:04X}=${t:02X}"
                            );
                        }
                    }
                    if frames == 4
                        || frames == 34
                        || frames == 35
                        || frames == 60
                        || frames == 120
                        || frames == 200
                        || frames == 300
                        || frames == 400
                        || frames == 500
                        || frames == 550
                        || frames == 593
                    {
                        eprintln!("{}", snapshot(&bus, &cpu, frames));
                        let map = &bus.ppu.vram.bank(0)[0x1800..0x1C00];
                        let real = map.iter().filter(|t| **t != 0 && **t != 0x7F).count();
                        eprintln!(
                            "  pal BG0={:04X} {:04X} {:04X} {:04X} OBJ0={:04X} {:04X} {:04X} {:04X} BGP={:02X} LCDC={:02X} WY={:02X} WX={:02X} map_real={real}/1024 hCGBPal={:02X}",
                            bus.ppu.cram.rgb555_bg(0, 0) & 0x7FFF,
                            bus.ppu.cram.rgb555_bg(0, 1) & 0x7FFF,
                            bus.ppu.cram.rgb555_bg(0, 2) & 0x7FFF,
                            bus.ppu.cram.rgb555_bg(0, 3) & 0x7FFF,
                            bus.ppu.cram.rgb555_obj(0, 0) & 0x7FFF,
                            bus.ppu.cram.rgb555_obj(0, 1) & 0x7FFF,
                            bus.ppu.cram.rgb555_obj(0, 2) & 0x7FFF,
                            bus.ppu.cram.rgb555_obj(0, 3) & 0x7FFF,
                            bus.read8(0xFF47),
                            bus.read8(0xFF40),
                            bus.read8(0xFF4A),
                            bus.read8(0xFF4B),
                            bus.read8(0xFFE5),
                        );
                    }
                    if frames >= 594 {
                        break;
                    }
                }
            }
            Err(e) => panic!("{e:?}"),
        }
    }

    eprintln!(
        "summary dma_starts={dma_starts} first_oam_zero={first_oam_zero:?} first_fb_zero={first_fb_zero:?} first_cram_change={first_cram_change:?} first_real_map={first_real_map:?} pal_flag_vblanks={pal_flag_on_vblank} key1={key1_seen:?} frames={frames}"
    );
}

const H_JOY_LAST: u16 = 0xFFA9;
const W_JUMPTABLE: u16 = 0xCF63;
const W_INTRO_FRAME: u16 = 0xCF64;
const W_REQ_A: u16 = 0xCF67;
const W_REQ_B: u16 = 0xCF6C;

/// Crystal intro progress: jumptable, joy skip, Serve2bpp LY window, map occupancy.
/// Diagnosis only — no hardware patch.
#[test]
#[ignore = "Crystal intro progress vs Serve2bpp LY window"]
fn crystal_intro_progress_before_title() {
    let path = Path::new(CART);
    if !path.exists() {
        eprintln!("SKIP missing {CART}");
        return;
    }

    let cart = Cartridge::load(path).expect("load Crystal");
    let mut cpu = Cpu::new();
    let mut bus = bus_from_cartridge(cart, HostHardwarePref::GameBoyColor).expect("force GBC");
    apply_fast(&mut cpu, &mut bus);

    let mut frames = 0u32;
    let mut in_vblank = false;
    let mut vblank_entry_ly = 0u8;
    let mut vblank_max_ly = 0u8;
    let mut first_joy_buttons: Option<(u32, u8, u16, u16)> = None;
    let mut last_jump = 0xFFu8;
    let mut serve_window_hits = 0u32;
    let mut serve_window_misses = 0u32;
    let mut pending_req = false;
    let mut serve2_hits = 0u32;
    let mut serve2_locked = 0u32;
    let max_steps = 1000u32.saturating_mul(70_224 / 4).saturating_mul(4);

    for _ in 0..max_steps {
        // Serve2bppRequest in bank 0 (home/video.asm).
        if cpu.pc == 0x1769 {
            serve2_hits += 1;
            let ly = bus.read8(0xFF44);
            let locked = !bus.ppu.vram_cpu_accessible();
            if locked {
                serve2_locked += 1;
            }
            if serve2_hits <= 8 || (frames >= 608 && serve2_hits.is_multiple_of(16)) || locked {
                eprintln!(
                    "Serve2bpp f={frames} LY={ly:02X} STAT={:02X} vram_ok={} req2={:02X} req1={:02X} pal={:02X} jump={:02X}",
                    bus.read8(0xFF41),
                    !locked,
                    bus.read8(W_REQ_A),
                    bus.read8(W_REQ_B),
                    bus.read8(0xFFE5),
                    bus.read8(W_JUMPTABLE),
                );
            }
        }
        if cpu.pc == 0x0040 {
            in_vblank = true;
            vblank_entry_ly = bus.read8(0xFF44);
            vblank_max_ly = vblank_entry_ly;
            let req = bus.read8(W_REQ_A) | bus.read8(W_REQ_B);
            pending_req = req != 0;
        }
        if in_vblank {
            let ly = bus.read8(0xFF44);
            if ly > vblank_max_ly && ly < 200 {
                vblank_max_ly = ly;
            }
            if bus.read8(cpu.pc) == 0xD9 {
                in_vblank = false;
                let in_window = vblank_entry_ly >= 144 && vblank_max_ly < 146;
                if pending_req {
                    if in_window {
                        serve_window_hits += 1;
                    } else {
                        serve_window_misses += 1;
                    }
                }
                if frames < 8 || frames.is_multiple_of(30) || !in_window && pending_req {
                    eprintln!(
                        "VBlank RETI f={frames} entryLY={vblank_entry_ly:02X} maxLY={vblank_max_ly:02X} reqA={:02X} reqB={:02X} pal={:02X} jump={:02X} joy={:02X} pending={pending_req} window={in_window}",
                        bus.read8(W_REQ_A),
                        bus.read8(W_REQ_B),
                        bus.read8(0xFFE5),
                        bus.read8(W_JUMPTABLE),
                        bus.read8(H_JOY_LAST),
                    );
                }
            }
        }

        let joy = bus.read8(H_JOY_LAST);
        if first_joy_buttons.is_none() && joy & 0x0F != 0 {
            first_joy_buttons = Some((frames, joy, cpu.pc, bus.cartridge.switchable_rom_bank()));
            eprintln!(
                "hJoyLast buttons f={frames} joy={joy:02X} PC={:04X} bank={:02X} P1={:02X}",
                cpu.pc,
                bus.cartridge.switchable_rom_bank(),
                bus.read8(0xFF00),
            );
        }

        match step(&mut cpu, &mut bus) {
            Ok(_) => {
                if bus.ppu.take_frame_ready() {
                    frames += 1;
                    let jump = bus.read8(W_JUMPTABLE);
                    let map = &bus.ppu.vram.bank(0)[0x1800..0x1C00];
                    let real = map.iter().filter(|t| **t != 0 && **t != 0x7F).count();
                    let (_, fb_nz) = rgb555_unique_nz(&bus);
                    if jump != last_jump {
                        eprintln!(
                            "jumptable {:02X}→{:02X} f={frames} intro={:02X} map_real={real} fb_nz={fb_nz} LCDC={:02X} joy={:02X} reqA={:02X} reqB={:02X} HDMA5={:02X}",
                            last_jump,
                            jump,
                            bus.read8(W_INTRO_FRAME),
                            bus.read8(0xFF40),
                            bus.read8(H_JOY_LAST),
                            bus.read8(W_REQ_A),
                            bus.read8(W_REQ_B),
                            bus.read8(HDMA5),
                        );
                        last_jump = jump;
                    }
                    if frames == 608
                        || frames == 650
                        || frames == 700
                        || frames == 800
                        || frames == 900
                        || frames == 950
                        || frames == 1000
                    {
                        let tiles = &bus.ppu.vram.bank(0)[0..16];
                        eprintln!("tiles0[0..16] f={frames} {}", hex16(tiles));
                    }
                    if frames.is_multiple_of(15)
                        || frames == 1
                        || frames == 60
                        || frames == 120
                        || frames == 608
                        || frames == 650
                        || frames == 800
                    {
                        eprintln!(
                            "f={frames} jump={jump:02X} intro={:02X} joy={:02X} P1={:02X} map_real={real} fb_nz={fb_nz} LCDC={:02X} WY={:02X} pal={:02X} reqA={:02X} reqB={:02X} HDMA5={:02X} PC={:04X} bank={:02X}",
                            bus.read8(W_INTRO_FRAME),
                            bus.read8(H_JOY_LAST),
                            bus.read8(0xFF00),
                            bus.read8(0xFF40),
                            bus.read8(0xFF4A),
                            bus.read8(0xFFE5),
                            bus.read8(W_REQ_A),
                            bus.read8(W_REQ_B),
                            bus.read8(HDMA5),
                            cpu.pc,
                            bus.cartridge.switchable_rom_bank(),
                        );
                    }
                    if frames >= 1000 {
                        break;
                    }
                }
            }
            Err(e) => panic!("{e:?}"),
        }
    }

    eprintln!(
        "summary frames={frames} first_joy={first_joy_buttons:?} serve_window_hits={serve_window_hits} serve_window_misses={serve_window_misses} serve2_pc_hits={serve2_hits} serve2_vram_locked={serve2_locked} last_jump={last_jump:02X}"
    );
}
