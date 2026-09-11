// Windows release: GUI subsystem so Explorer double-click opens only the app
// window (no companion console). Debug builds keep the console subsystem.
// See `windows_console` and issue #15.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod frontend;
mod windows_console;

use frontend::{LaunchRom, Verbosity, is_rom_path, run as run_window, run_tone_test};
use graycart::{
    BootMode, Cartridge, Cpu, ExecSession, HostHardwarePref, RunOutcome, apply_fast,
    bus_from_cartridge, default_save_path, disassemble, flush_save, format_trace_line, hex_dump,
    legacy_sidecar_save_path, load_save, load_save_with_fallback,
};
use std::env;
use std::path::{Path, PathBuf};
use std::process;

fn main() {
    // Reattach to the launching terminal for CLI/headless stdout on GUI-subsystem
    // release builds. No-op when there is no parent console (Explorer launch).
    #[cfg(all(windows, not(debug_assertions)))]
    windows_console::attach_parent_console();

    let mut argv: Vec<String> = env::args().skip(1).collect();
    if argv.first().map(|s| s.as_str()) == Some("--audio-test") {
        argv.remove(0);
        let mut seconds = 2.0f32;
        let mut i = 0;
        while i < argv.len() {
            match argv[i].as_str() {
                "--seconds" => {
                    i += 1;
                    let Some(n) = argv.get(i) else {
                        eprintln!("--seconds requires a number");
                        process::exit(1);
                    };
                    seconds = n.parse().unwrap_or_else(|_| {
                        eprintln!("invalid --seconds value: {n}");
                        process::exit(1);
                    });
                }
                "--help" | "-h" => {
                    eprintln!(
                        "usage: graycart --audio-test [--seconds <n>]\n\
                         Plays a 440 Hz square through the host device (bypasses APU)."
                    );
                    process::exit(0);
                }
                other => {
                    eprintln!("unknown audio-test argument: {other}");
                    process::exit(1);
                }
            }
            i += 1;
        }
        if let Err(e) = run_tone_test(seconds) {
            eprintln!("{e}");
            process::exit(1);
        }
        return;
    }

    // No args → application window (empty ROM launcher).
    if argv.is_empty() {
        open_gui(None, BootMode::BootRom, Verbosity::Normal, false, None);
        return;
    }

    let mut args = argv.into_iter().peekable();

    // Flags-only (e.g. `graycart --run`) → empty GUI.
    if args
        .peek()
        .is_some_and(|a| a.starts_with('-') && a.as_str() != "-")
    {
        let mut unthrottled = false;
        let mut skip_boot = false;
        let mut verbosity = Verbosity::Normal;
        let mut want_run = false;
        let mut hardware_cli = None;
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--run" => want_run = true,
                "--unthrottled" => unthrottled = true,
                "--skip-boot" => skip_boot = true,
                "--quiet" => verbosity = Verbosity::Quiet,
                "--verbose" => verbosity = Verbosity::Verbose,
                "--hardware" => {
                    let Some(v) = args.next() else {
                        eprintln!(
                            "--hardware requires automatic, game-boy, or game-boy-color (aliases: dmg, cgb)"
                        );
                        process::exit(1);
                    };
                    hardware_cli = Some(parse_hardware_pref(&v));
                }
                "--help" | "-h" => {
                    usage();
                    process::exit(0);
                }
                "--version" | "-V" => {
                    println!("graycart {}", env!("CARGO_PKG_VERSION"));
                    process::exit(0);
                }
                other => {
                    eprintln!("unknown argument (ROM path required for this flag): {other}");
                    usage();
                    process::exit(1);
                }
            }
        }
        if !want_run && !unthrottled && !skip_boot {
            // Bare `--help` already handled; other flag-only without --run is odd.
            usage();
            process::exit(1);
        }
        let boot = if skip_boot {
            BootMode::Fast
        } else {
            BootMode::BootRom
        };
        open_gui(None, boot, verbosity, unthrottled, hardware_cli);
        return;
    }

    let path = args.next().unwrap();
    let mut disasm_addr: Option<usize> = None;
    let mut count: usize = 32;
    let mut trace_steps: Option<usize> = None;
    let mut run = false;
    let mut unthrottled = false;
    let mut skip_boot = false;
    let mut frame_cap: Option<u64> = None;
    let mut verbosity = Verbosity::Normal;
    let mut save_path_arg: Option<PathBuf> = None;
    let mut hardware_cli = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--disasm" => {
                let Some(addr) = args.next() else {
                    eprintln!("--disasm requires an address (e.g. 0x0150)");
                    process::exit(1);
                };
                disasm_addr = Some(parse_addr(&addr));
            }
            "--count" => {
                let Some(n) = args.next() else {
                    eprintln!("--count requires a number");
                    process::exit(1);
                };
                count = parse_usize(&n, "--count");
            }
            "--trace" => {
                let Some(n) = args.next() else {
                    eprintln!("--trace requires a step count");
                    process::exit(1);
                };
                trace_steps = Some(parse_usize(&n, "--trace"));
            }
            "--frames" => {
                let Some(n) = args.next() else {
                    eprintln!("--frames requires a frame count");
                    process::exit(1);
                };
                frame_cap = Some(parse_u64(&n, "--frames"));
            }
            "--save" => {
                let Some(p) = args.next() else {
                    eprintln!("--save requires a path");
                    process::exit(1);
                };
                save_path_arg = Some(PathBuf::from(p));
            }
            "--hardware" => {
                let Some(v) = args.next() else {
                    eprintln!(
                        "--hardware requires automatic, game-boy, or game-boy-color (aliases: dmg, cgb)"
                    );
                    process::exit(1);
                };
                hardware_cli = Some(parse_hardware_pref(&v));
            }
            "--quiet" => verbosity = Verbosity::Quiet,
            "--verbose" => verbosity = Verbosity::Verbose,
            "--run" => run = true,
            "--unthrottled" => unthrottled = true,
            "--skip-boot" => skip_boot = true,
            "--audio-test" => {
                eprintln!("--audio-test must be first (no ROM): graycart --audio-test");
                process::exit(1);
            }
            "--help" | "-h" => {
                usage();
                process::exit(0);
            }
            "--version" | "-V" => {
                println!("graycart {}", env!("CARGO_PKG_VERSION"));
                process::exit(0);
            }
            other => {
                eprintln!("unknown argument: {other}");
                usage();
                process::exit(1);
            }
        }
    }

    if unthrottled && !run {
        eprintln!("--unthrottled only applies with --run");
        process::exit(1);
    }

    let headless = frame_cap.is_some() || trace_steps.is_some() || disasm_addr.is_some();
    if (run || headless) && !is_rom_path(Path::new(&path)) {
        eprintln!("unsupported ROM extension for {path} (expected .gb, .gbc, .rom, or .bin)");
        process::exit(1);
    }

    let mut cart = match Cartridge::load(&path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("failed to load {path}: {e}");
            process::exit(1);
        }
    };

    if !cart.header_ok() {
        eprintln!("invalid cartridge header (logo or header checksum failed)");
        process::exit(1);
    }

    let explicit_save = save_path_arg.is_some();
    let save_path = save_path_arg.unwrap_or_else(|| default_save_path(&path));
    let persist = run || frame_cap.is_some();
    if persist {
        let load_result = if explicit_save {
            load_save(&save_path, &mut cart)
        } else {
            let legacy = legacy_sidecar_save_path(&path);
            load_save_with_fallback(&save_path, Some(&legacy), &mut cart)
        };
        match load_result {
            Ok(true) => {
                if verbosity != Verbosity::Quiet {
                    if explicit_save || save_path.exists() {
                        println!("save: loaded {}", save_path.display());
                    } else {
                        let legacy = legacy_sidecar_save_path(&path);
                        println!(
                            "save: loaded {} (legacy sidecar; next flush → {})",
                            legacy.display(),
                            save_path.display()
                        );
                    }
                }
            }
            Ok(false) => {
                if verbosity == Verbosity::Verbose && cart.has_battery_backed_ram() {
                    println!("save: no file at {} (fresh SRAM)", save_path.display());
                }
            }
            Err(e) => {
                eprintln!("failed to load save {}: {e}", save_path.display());
                process::exit(1);
            }
        }
    }

    match verbosity {
        Verbosity::Quiet => {
            println!("loaded {}", cart.header.title.trim());
        }
        Verbosity::Normal | Verbosity::Verbose => {
            println!("loaded {path}");
            println!("size: {} bytes", cart.len());
            println!("{}", cart.header);
        }
    }

    let hardware_pref = hardware_cli.unwrap_or(HostHardwarePref::Automatic);

    let mut cpu = Cpu::new();
    let boot_mode = if run && !skip_boot {
        BootMode::BootRom
    } else {
        BootMode::Fast
    };
    if verbosity != Verbosity::Quiet {
        println!("boot: {}", boot_mode.label());
    }

    if let Some(n) = frame_cap {
        if run {
            eprintln!("--frames is headless; omit --run");
            process::exit(1);
        }
        let mut bus = bus_from_cartridge_or_exit(cart, hardware_pref);
        if verbosity != Verbosity::Quiet {
            println!(
                "hardware: {} ({})",
                bus.hardware_model().launch_label(),
                hardware_pref.cli_name()
            );
        }
        apply_fast(&mut cpu, &mut bus);
        let mut session = ExecSession::new();
        match session.run_frames(&mut cpu, &mut bus, n) {
            RunOutcome::FrameLimit { frames, steps } => {
                println!("frames: {frames}/{n}");
                if verbosity == Verbosity::Verbose {
                    println!("steps: {steps}");
                }
            }
            RunOutcome::Fault(report) => {
                eprintln!("{report}");
                let _ = flush_save_report(&save_path, &mut bus.cartridge, verbosity);
                process::exit(1);
            }
        }
        if let Err(e) = flush_save_report(&save_path, &mut bus.cartridge, verbosity) {
            eprintln!("{e}");
            process::exit(1);
        }
        return;
    }

    if run {
        open_gui(
            Some(LaunchRom {
                path: PathBuf::from(path),
                save_path,
                cart,
            }),
            boot_mode,
            verbosity,
            unthrottled,
            hardware_cli,
        );
        return;
    }

    // Non-run paths: fast handoff for dump/trace.
    let mut bus = bus_from_cartridge_or_exit(cart, hardware_pref);
    if verbosity != Verbosity::Quiet {
        println!(
            "hardware: {} ({})",
            bus.hardware_model().launch_label(),
            hardware_pref.cli_name()
        );
    }
    apply_fast(&mut cpu, &mut bus);
    if verbosity == Verbosity::Verbose {
        println!("cpu (after boot): {cpu}");
    }

    if let Some(steps) = trace_steps {
        let mut session = ExecSession::new();
        if verbosity != Verbosity::Quiet {
            println!("--- cpu trace ({steps} steps) ---");
        }
        for _ in 0..steps {
            let pc = cpu.pc;
            match session.step(&mut cpu, &mut bus) {
                Ok(decoded) => {
                    if decoded.len > 0 || verbosity == Verbosity::Verbose {
                        println!(
                            "{}",
                            format_trace_line(session.steps.saturating_sub(1), pc, &decoded, &cpu)
                        );
                    }
                }
                Err(e) => {
                    eprintln!("{}", session.fault_from_step(e, &cpu, &bus));
                    process::exit(1);
                }
            }
        }
        return;
    }

    match disasm_addr {
        Some(addr) => {
            println!("--- disasm @ ${addr:04X} ({count} instr) ---");
            disassemble(&bus.cartridge.rom, addr, count);
        }
        None => {
            println!("--- hex dump $0100..$014F ---");
            hex_dump(&bus.cartridge.rom, 0x0100, 0x0150);
            println!("--- disasm @ $0100 ---");
            disassemble(&bus.cartridge.rom, 0x0100, 8);
            println!("hint: graycart");
            println!("hint: graycart {path} --run");
            println!("hint: graycart {path} --run --skip-boot");
            println!("hint: graycart {path} --run --unthrottled");
            println!("hint: graycart {path} --frames 30");
            println!("hint: graycart {path} --frames 30 --quiet");
            println!("hint: graycart {path} --save path.sav --run");
            println!("hint: graycart {path} --trace 3");
            println!("hint: graycart {path} --disasm 0x0150");
        }
    }
}

fn open_gui(
    initial: Option<LaunchRom>,
    boot_mode: BootMode,
    verbosity: Verbosity,
    unthrottled: bool,
    hardware_cli: Option<HostHardwarePref>,
) {
    if let Err(e) = run_window(initial, boot_mode, verbosity, unthrottled, hardware_cli) {
        eprintln!("{e}");
        process::exit(1);
    }
}

fn flush_save_report(
    save_path: &Path,
    cart: &mut Cartridge,
    verbosity: Verbosity,
) -> Result<(), String> {
    match flush_save(save_path, cart) {
        Ok(true) => {
            if verbosity != Verbosity::Quiet {
                println!("save: wrote {}", save_path.display());
            }
            Ok(())
        }
        Ok(false) => Ok(()),
        Err(e) => Err(format!("failed to write save {}: {e}", save_path.display())),
    }
}

fn parse_hardware_pref(s: &str) -> HostHardwarePref {
    match s {
        "automatic" => HostHardwarePref::Automatic,
        "game-boy" | "dmg" => HostHardwarePref::GameBoy,
        "game-boy-color" | "cgb" => HostHardwarePref::GameBoyColor,
        other => {
            eprintln!(
                "invalid --hardware value: {other} (expected automatic, game-boy, game-boy-color, dmg, or cgb)"
            );
            process::exit(1);
        }
    }
}

fn bus_from_cartridge_or_exit(cart: Cartridge, pref: HostHardwarePref) -> graycart::Bus {
    match bus_from_cartridge(cart, pref) {
        Ok(bus) => bus,
        Err(e) => {
            eprintln!("{e}");
            process::exit(1);
        }
    }
}

fn parse_usize(s: &str, flag: &str) -> usize {
    s.parse().unwrap_or_else(|_| {
        eprintln!("invalid {flag} value: {s}");
        process::exit(1);
    })
}

fn parse_u64(s: &str, flag: &str) -> u64 {
    s.parse().unwrap_or_else(|_| {
        eprintln!("invalid {flag} value: {s}");
        process::exit(1);
    })
}

fn parse_addr(s: &str) -> usize {
    let s = s.trim().trim_start_matches('$');
    let (s, radix) = if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        (hex, 16)
    } else if s.chars().any(|c| matches!(c, 'a'..='f' | 'A'..='F')) {
        (s, 16)
    } else if let Some(dec) = s.strip_prefix('+') {
        return dec.parse().unwrap_or_else(|_| {
            eprintln!("invalid address: {s}");
            process::exit(1);
        });
    } else {
        (s, 16)
    };

    usize::from_str_radix(s, radix).unwrap_or_else(|_| {
        eprintln!("invalid address: {s}");
        process::exit(1);
    })
}

fn usage() {
    eprintln!(
        "graycart {}\n\
         usage: graycart [<rom.gb>] [--run] [--skip-boot] [--unthrottled] [--save <path.sav>] [--hardware automatic|game-boy|game-boy-color] [--frames <n>] [--quiet|--verbose] [--disasm <addr>] [--count <n>] [--trace <steps>]\n\
         graycart --audio-test [--seconds <n>]\n\
         graycart --version\n\
         \n\
         examples:\n\
           graycart\n\
           graycart --run\n\
           graycart rom.gb --run\n\
           graycart rom.gb --run --skip-boot\n\
           graycart rom.gb --run --unthrottled\n\
           graycart rom.gb --save custom.sav --run\n\
           graycart rom.gb --run --verbose\n\
           graycart --audio-test\n\
           graycart rom.gb --frames 30 --quiet\n\
           graycart rom.gbc --run --skip-boot --hardware game-boy-color\n\
           graycart rom.gb --hardware game-boy-color --frames 30\n\
           graycart rom.gb --disasm 0x0150\n\
           graycart rom.gb --trace 3",
        env!("CARGO_PKG_VERSION")
    );
}
