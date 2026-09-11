use super::*;
use postcard;

#[test]
fn machine_state_v1_postcard_roundtrip_empty_serial() {
    let state = MachineStateV1 {
        cpu: CpuStateV1 {
            a: 0x01,
            f: 0xB0,
            b: 0,
            c: 0x13,
            d: 0,
            e: 0xD8,
            h: 0,
            l: 0x4D,
            sp: 0xFFFE,
            pc: 0x0100,
            ime: false,
            ime_enable_pending: false,
            ime_enable_armed: false,
            halted: false,
        },
        bus: BusRamStateV1 {
            vram: [0; 0x2000],
            wram: [0; 0x2000],
            io: [0; 0x80],
            hram: [0; 0x7F],
            ie: 0,
            serial: Vec::new(),
            oam_dma_suppress_t: 0,
        },
        cart: CartStateV1 {
            ram: vec![0xAA; 0x2000],
            mapper: MapperStateV1::None,
        },
        timer: TimerStateV1 {
            div: 0xAB00,
            tima: 0,
            tma: 0,
            tac: 0,
            reload_delay: 0,
        },
        ppu: PpuStateV1 {
            lcdc: 0x91,
            stat: 0x85,
            scy: 0,
            scx: 0,
            ly: 0,
            lyc: 0,
            dma: 0xFF,
            bgp: 0xFC,
            obp0: 0xFF,
            obp1: 0xFF,
            wy: 0,
            wx: 0,
            mode: 2,
            dot: 0,
            window_line: 0,
            window_y_active: false,
            oam: [0xFF; 0xA0],
            dma_active: false,
            dma_src: 0,
            dma_index: 0,
            dma_delay: 0,
        },
        apu: ApuStateV1 {
            powered: true,
            frame_seq_step: 0,
            ch1: Ch1StateV1 {
                square: SquareStateV1 {
                    enabled: false,
                    dac_on: false,
                    length: 0,
                    length_enable: false,
                    duty: 0,
                    envelope_volume: 0,
                    envelope_dir: false,
                    envelope_period: 0,
                    envelope_timer: 0,
                    frequency: 0,
                    frequency_timer: 0,
                    duty_pos: 0,
                },
                sweep_period: 0,
                sweep_negate: false,
                sweep_shift: 0,
                sweep_timer: 0,
                sweep_enable: false,
            },
            ch2: SquareStateV1 {
                enabled: false,
                dac_on: false,
                length: 0,
                length_enable: false,
                duty: 0,
                envelope_volume: 0,
                envelope_dir: false,
                envelope_period: 0,
                envelope_timer: 0,
                frequency: 0,
                frequency_timer: 0,
                duty_pos: 0,
            },
            ch3: WaveStateV1 {
                dac_on: false,
                length: 0,
                length_enable: false,
                volume_code: 0,
                frequency: 0,
                frequency_timer: 0,
                pos_nib: 0,
            },
            ch4: NoiseStateV1 {
                enabled: false,
                dac_on: false,
                length: 0,
                length_enable: false,
                envelope_volume: 0,
                envelope_dir: false,
                envelope_period: 0,
                envelope_timer: 0,
                clock_shift: 0,
                width_mode: false,
                divisor_code: 0,
                lfsr: 0x7FFF,
                timer: 0,
            },
            nr50: 0x77,
            nr51: 0xF3,
            wave_ram: [0; 16],
            sample_phase: 0.0,
        },
        joypad: JoypadStateV1 {
            select: 0,
            pressed: 0,
        },
        runtime: RuntimeStateV1 { post_boot: true },
    };
    let bytes = postcard::to_allocvec(&state).expect("encode");
    let decoded: MachineStateV1 = postcard::from_bytes(&bytes).expect("decode");
    assert_eq!(decoded, state);
    assert!(state.heap_bytes() >= 0x2000);
}

#[test]
fn snapshot_format_version_is_one() {
    assert_eq!(SNAPSHOT_FORMAT_VERSION, 1);
}

mod capture_restore {
    use super::*;
    use crate::{Bus, Cartridge, Cpu, Header, apply_fast, step};

    fn minimal_rom() -> Vec<u8> {
        let mut rom = vec![0u8; 0x8000];
        rom[0x0100] = 0x00; // NOP
        rom[0x0101] = 0x18; // JR
        rom[0x0102] = 0xFE; // -2 (spin)
        crate::Cartridge::rom_only(rom).rom.clone()
    }

    fn mbc1_battery_rom() -> Vec<u8> {
        let mut rom = vec![0xFF; 0x8000];
        rom[0x0100] = 0x00; // NOP
        rom[0x0101] = 0x18; // JR
        rom[0x0102] = 0xFE; // -2 (spin)
        rom[0x0147] = 0x03; // MBC1+RAM+BATTERY
        rom[0x0148] = 0x00; // 32 KiB
        rom[0x0149] = 0x02; // 8 KiB RAM
        rom
    }

    fn bus_from_rom_bytes(rom: Vec<u8>) -> Bus {
        let header = Header::parse(&rom).expect("test rom header");
        let cart = Cartridge::from_parts(rom, header).expect("test cart");
        Bus::new(cart)
    }

    fn run_to_first_vblank(cpu: &mut Cpu, bus: &mut Bus) {
        let mut steps = 0u32;
        loop {
            step(cpu, bus).expect("step");
            steps += 1;
            if bus.ppu.take_frame_ready() {
                break;
            }
            assert!(steps < 500_000, "timeout waiting for vblank");
        }
    }

    #[test]
    fn capture_restore_roundtrip_preserves_pc_and_ram() {
        let rom = minimal_rom();
        let mut cpu = Cpu::new();
        let mut bus = Bus::from_rom(rom);
        apply_fast(&mut cpu, &mut bus);
        run_to_first_vblank(&mut cpu, &mut bus);

        let before = capture(&cpu, &bus);
        let saved_pc = cpu.pc;
        let saved_wram0 = bus.read8(0xC000);

        cpu.pc = 0xDEAD;
        bus.write8(0xC000, saved_wram0 ^ 0xFF);

        restore(&before, &mut cpu, &mut bus);
        assert_eq!(cpu.pc, saved_pc);
        assert_eq!(bus.read8(0xC000), saved_wram0);
    }

    #[test]
    fn restore_marks_battery_dirty_when_sram_present() {
        let mut cpu = Cpu::new();
        let mut bus = bus_from_rom_bytes(mbc1_battery_rom());
        assert!(bus.cartridge.has_battery_backed_ram());
        apply_fast(&mut cpu, &mut bus);
        run_to_first_vblank(&mut cpu, &mut bus);

        bus.write8(0x0000, 0x0A); // MBC1 RAM enable
        bus.write8(0xA000, 0x42);
        let snap = capture(&cpu, &bus);
        bus.cartridge.clear_save_dirty();
        assert!(!bus.cartridge.is_save_dirty());

        restore(&snap, &mut cpu, &mut bus);
        assert!(bus.cartridge.is_save_dirty());
        assert_eq!(bus.read8(0xA000), 0x42);
    }

    #[test]
    fn determinism_after_restore() {
        let rom = minimal_rom();
        let mut cpu_a = Cpu::new();
        let mut bus_a = Bus::from_rom(rom.clone());
        let mut cpu_b = Cpu::new();
        let mut bus_b = Bus::from_rom(rom);
        apply_fast(&mut cpu_a, &mut bus_a);
        apply_fast(&mut cpu_b, &mut bus_b);

        for _ in 0..3 {
            run_to_first_vblank(&mut cpu_a, &mut bus_a);
            run_to_first_vblank(&mut cpu_b, &mut bus_b);
        }

        let snap = capture(&cpu_a, &bus_a);
        restore(&snap, &mut cpu_b, &mut bus_b);

        for _ in 0..2 {
            run_to_first_vblank(&mut cpu_a, &mut bus_a);
            run_to_first_vblank(&mut cpu_b, &mut bus_b);
        }

        assert_eq!(cpu_a.pc, cpu_b.pc);
        assert_eq!(cpu_a.a, cpu_b.a);
        assert_eq!(bus_a.ppu.ly(), bus_b.ppu.ly());
    }
}

mod gcs1_format {
    use super::format::*;
    use super::*;
    use crate::{Bus, Cartridge, Cpu, apply_fast, step};

    fn minimal_rom() -> Vec<u8> {
        let mut rom = vec![0u8; 0x8000];
        rom[0x0100] = 0x00; // NOP
        rom[0x0101] = 0x18; // JR
        rom[0x0102] = 0xFE; // -2 (spin)
        Cartridge::rom_only(rom).rom.clone()
    }

    fn run_to_first_vblank(cpu: &mut Cpu, bus: &mut Bus) {
        let mut steps = 0u32;
        loop {
            step(cpu, bus).expect("step");
            steps += 1;
            if bus.ppu.take_frame_ready() {
                break;
            }
            assert!(steps < 500_000, "timeout waiting for vblank");
        }
    }

    #[test]
    fn gcs1_roundtrip_encode_decode() {
        let rom = minimal_rom();
        let state = {
            let mut cpu = Cpu::new();
            let mut bus = Bus::from_rom(rom.clone());
            apply_fast(&mut cpu, &mut bus);
            run_to_first_vblank(&mut cpu, &mut bus);
            capture(&cpu, &bus)
        };
        let bytes = encode_gcs1(&rom, "TESTGAME", &state);
        let (decoded, meta) = decode_gcs1(&rom, &bytes).expect("decode");
        assert_eq!(decoded, state);
        assert_eq!(meta.title, "TESTGAME");
    }

    #[test]
    fn gcs1_rejects_wrong_rom_sha() {
        let rom_a = minimal_rom();
        let mut rom_b = minimal_rom();
        rom_b[0x0147] = 0x01;
        let state = {
            let mut cpu = Cpu::new();
            let mut bus = Bus::from_rom(rom_a.clone());
            apply_fast(&mut cpu, &mut bus);
            capture(&cpu, &bus)
        };
        let bytes = encode_gcs1(&rom_a, "TEST", &state);
        let err = decode_gcs1(&rom_b, &bytes).unwrap_err();
        assert!(matches!(err, Gcs1Error::RomMismatch));
    }

    #[test]
    fn gcs1_rejects_truncated() {
        let rom = minimal_rom();
        let state = {
            let cpu = Cpu::new();
            let bus = Bus::from_rom(rom.clone());
            capture(&cpu, &bus)
        };
        let mut bytes = encode_gcs1(&rom, "TEST", &state);
        bytes.pop();
        let err = decode_gcs1(&rom, &bytes).unwrap_err();
        assert!(matches!(err, Gcs1Error::Truncated));
    }

    #[test]
    fn gcs1_rejects_corrupt_payload_crc() {
        let rom = minimal_rom();
        let state = {
            let cpu = Cpu::new();
            let bus = Bus::from_rom(rom.clone());
            capture(&cpu, &bus)
        };
        let mut bytes = encode_gcs1(&rom, "TEST", &state);
        let last = bytes.len() - 1;
        bytes[last] ^= 0xFF;
        let err = decode_gcs1(&rom, &bytes).unwrap_err();
        assert!(matches!(err, Gcs1Error::PayloadCrcMismatch));
    }

    #[test]
    fn gcs1_rejects_unsupported_version() {
        let rom = minimal_rom();
        let state = {
            let cpu = Cpu::new();
            let bus = Bus::from_rom(rom.clone());
            capture(&cpu, &bus)
        };
        let mut bytes = encode_gcs1(&rom, "TEST", &state);
        bytes[4..6].copy_from_slice(&99u16.to_le_bytes());
        let err = decode_gcs1(&rom, &bytes).unwrap_err();
        assert!(matches!(err, Gcs1Error::UnsupportedVersion(99)));
    }

    #[test]
    fn gcs1_rejects_bad_magic() {
        let rom = minimal_rom();
        let state = {
            let cpu = Cpu::new();
            let bus = Bus::from_rom(rom.clone());
            capture(&cpu, &bus)
        };
        let mut bytes = encode_gcs1(&rom, "TEST", &state);
        bytes[0] = b'X';
        assert!(matches!(
            decode_gcs1(&rom, &bytes).unwrap_err(),
            Gcs1Error::BadMagic
        ));
    }

    #[test]
    fn sanitize_title_strips_unsafe_filename_chars() {
        assert_eq!(sanitize_title("Pokemon Red"), "Pokemon Red");
        assert_eq!(
            sanitize_title("Zelda: Link's Awakening"),
            "Zelda Link s Awakening"
        );
        assert_eq!(sanitize_title("  "), "Graycart");
    }
}

#[test]
#[ignore = "manual release benchmark — run with --ignored --nocapture"]
fn rewind_capture_restore_benchmark() {
    use crate::{Bus, Cpu, apply_fast, capture, restore, step};
    use std::time::Instant;

    const REWIND_SOFT_CAP_BYTES: usize = 32 * 1024 * 1024;

    fn minimal_rom() -> Vec<u8> {
        let mut rom = vec![0u8; 0x8000];
        rom[0x0100] = 0x00;
        rom[0x0101] = 0x18;
        rom[0x0102] = 0xFE;
        crate::Cartridge::rom_only(rom).rom.clone()
    }

    fn run_to_first_vblank(cpu: &mut Cpu, bus: &mut Bus) {
        let mut steps = 0u32;
        loop {
            step(cpu, bus).expect("step");
            steps += 1;
            if bus.ppu.take_frame_ready() {
                break;
            }
            assert!(steps < 500_000, "timeout waiting for vblank");
        }
    }

    let rom = minimal_rom();
    let mut cpu = Cpu::new();
    let mut bus = Bus::from_rom(rom);
    apply_fast(&mut cpu, &mut bus);
    run_to_first_vblank(&mut cpu, &mut bus);

    let snap = capture(&cpu, &bus);
    let size_kib = snap.heap_bytes() as f64 / 1024.0;

    let n = 200u32;
    let t0 = Instant::now();
    for _ in 0..n {
        let _ = capture(&cpu, &bus);
    }
    let capture_ms = t0.elapsed().as_secs_f64() * 1000.0 / f64::from(n);

    let t1 = Instant::now();
    for _ in 0..n {
        restore(&snap, &mut cpu, &mut bus);
    }
    let restore_ms = t1.elapsed().as_secs_f64() * 1000.0 / f64::from(n);

    let frames_at_cap = (REWIND_SOFT_CAP_BYTES as f64 / snap.heap_bytes() as f64).floor();
    let seconds = frames_at_cap / 60.0;

    eprintln!("snapshot size:       {size_kib:.1} KiB");
    eprintln!("capture avg:         {capture_ms:.3} ms");
    eprintln!("restore avg:         {restore_ms:.3} ms");
    eprintln!("32 MiB rewind:       {seconds:.1} sec @ 60 Hz (N=1)");

    assert!(
        capture_ms < 5.0,
        "investigate if >5ms — target <0.5ms for default-on promotion"
    );
}
