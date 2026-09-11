# Graycart

DMG and Game Boy Color emulator. This repository is **`graycart-gb`** in the [Graycart family](https://github.com/graycart/graycart). The crate and binary are named **`graycart`**.

**CGB silicon is on** (`CGB_SILICON_READY`). Emulation → Hardware **Automatic** uses CGB hardware. You can still force original Game Boy or Game Boy Color.

This is **0.11.1**, not 1.0.0. Playable CGB needs KEY1 + HDMA (landed). Packaging, trusted settings migration, and save/state compatibility are still open.

## Status

- **DMG:** playable; Blargg `cpu_instrs` and `dmg_sound` are the CPU/APU accuracy gates.
- **CGB:** palettes, attributes, priority, double-speed, GDMA/HDMA, CGB boot mapping.
- **Mappers:** MBC0 / MBC1 / MBC2 / MBC3 (+ RTC) / MBC5. Unsupported types fail loudly (no silent ROM-only).
- **Host:** Windows, macOS, Linux. Release builds are the supported play mode; debug is not expected to hold realtime.

## Build and run

Rust stable (see CI). From this repo:

```bash
cargo run --release
cargo run --release -- path/to/game.gb
```

Headless:

```bash
cargo run --release -- --frames 120 path/to/game.gb
```

`cargo test` is **Rust unit and integration tests only** (no test-ROM download). ROM matrices stay in this repo: `cargo test --test roms … -- --ignored`.

Required before a change is done:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

## Hardware mode

`$0143` is cartridge capability, not a 1:1 hardware switch. `.gbc` is a filename hint only.

| Menu / `--hardware` | Meaning |
|---------------------|---------|
| Automatic | CGB silicon (default) |
| Game Boy | DMG |
| Game Boy Color | CGB |

## Boot ROM

Nintendo firmware is **not** in this repository. Fast boot is the default.

Optional official `dmg_boot.bin` (256 bytes) and CGB boot (2048 bytes) can be installed from the UI (boot firmware cache next to the executable). Do not commit `*.bin` boot images.

## Controls (defaults)

Keyboard: arrows = D-pad, **Z** = B, **X** = A, Backspace = Select, Enter = Start.

Host: F5 quick save, F8 quick load, F6 screenshot, hold R rewind, F11 fullscreen, F12 debug monitor, F9 capture.

**Configure Controls** is a separate window. It does not pause the emulator or treat the session as unfocused.

## Audio

Release builds open a host output stream (CPAL). **System Default** prefers the OS endpoint (WASAPI / Core Audio / Pulse+ALSA bridges), then CPAL default, then a loud fallback (never saved as your choice), else audio offline.

Audio → device list is cached; enumerating WASAPI/CPAL every frame is not done.

## Saves, states, rewind

- Battery `.sav` next to the ROM (load on start, flush on exit). Keep dumps out of git.
- State slots `{rom}.gcs0`–`.gcs9` (GCS1).
- In-memory rewind ring (hold R).

## Diagnostics

Debug monitor (F12) observes; it does not own emulation. Snapshots are produced on the emu thread.

## Tests

See [docs/conformance.md](docs/conformance.md).

## License

[MIT](LICENSE) — Copyright (c) 2026 Graycart.

Blargg / Mooneye / Acid2 ROMs keep their **upstream** licenses. Do not add commercial cartridges to git.
