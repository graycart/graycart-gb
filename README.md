# Graycart

DMG and Game Boy Color emulator. This repository is **`graycart-gb`** in the [Graycart family](https://github.com/graycart/graycart). The crate and binary are named **`graycart`**.

CGB silicon is on (`CGB_SILICON_READY`). Hardware mode **Automatic** runs as Game Boy Color; you can still force original Game Boy or Game Boy Color.

Version **0.11.4** — not 1.0. KEY1 and HDMA are in for playable CGB. Packaging, trusted settings migration, and save/state compatibility are still open.

## Status

- **DMG:** playable. Blargg `cpu_instrs` and `dmg_sound` are the CPU/APU accuracy gates.
- **CGB:** palettes, attributes, priority, double-speed, GDMA/HDMA, CGB boot mapping.
- **Mappers:** MBC0 / MBC1 / MBC2 / MBC3 (+ RTC) / MBC5. Unsupported cartridge types fail loudly instead of pretending to be ROM-only.
- **Host:** Windows, macOS, Linux. Use a release build for play; debug builds are not expected to keep realtime.

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

Tagged builds: [GitHub Releases](https://github.com/graycart/graycart-gb/releases).

**Windows release note:** the packaged `.exe` is a GUI-subsystem binary (no console window on double-click). Debug `cargo run` keeps a console. Headless/CLI flags (`--frames`, `--version`, …) still print when launched from a terminal — the process reattaches to the parent console.

`cargo test` covers Rust unit and integration tests only — it does not download test ROMs. Optional accuracy matrices use in-repo fixtures under [`tests/fixtures/`](tests/fixtures/); details in [docs/conformance.md](docs/conformance.md):

```bash
cargo test --test roms blargg_cpu_instrs_matrix -- --ignored
```

Before you call a change done:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

## Hardware mode

Cartridge header `$0143` is capability, not a 1:1 hardware switch. A `.gbc` extension is only a filename hint.

| Menu / `--hardware` | Meaning |
|---------------------|---------|
| Automatic | CGB silicon (default) |
| Game Boy | DMG |
| Game Boy Color | CGB |

## Boot ROM

Nintendo firmware is not in this repository. Fast boot is the default.

You can install official `dmg_boot.bin` (256 bytes) and CGB boot (2048 bytes) from the UI; they live in a boot-firmware cache next to the executable. Do not commit `*.bin` boot images.

## Controls (defaults)

Keyboard: arrows = D-pad, **Z** = B, **X** = A, Backspace = Select, Enter = Start.

Host: F5 quick save, F8 quick load, F6 screenshot, hold R rewind, F11 fullscreen, F12 debug monitor, F9 capture.

**Configure Controls** opens in its own window. It does not pause the emulator or treat the session as unfocused.

The debug monitor (F12) observes only; it does not drive emulation. Snapshots are taken on the emu thread.

## Audio

Release builds open a host output stream (CPAL). **System Default** prefers the OS endpoint (WASAPI / Core Audio / Pulse+ALSA bridges), then the CPAL default, then a loud fallback that is never saved as your choice — otherwise audio stays offline.

The Audio menu caches the device list so WASAPI/CPAL are not re-enumerated every frame.

## Saves, states, rewind

- Battery `.sav` files live under the Graycart data directory (`…/Graycart/saves/{stem}.sav` via `dirs::data_dir`). The directory is created on first flush. Existing ROM-adjacent `.sav` files still load as a migration fallback; the next flush writes only to the dedicated path (no dual-write). `--save <path>` always wins. Keep dumps out of git.
- State slots `{rom}.gcs0`–`.gcs9` (GCS1).
- In-memory rewind ring (hold R).

## Reporting issues

Help → **Report bug…** / **Request feature…** (and crash consent) only file after an explicit Send. A fine-grained GitHub PAT is optional — Help → **Optional GitHub token…** or `GRAYCART_GITHUB_TOKEN`. Without a token, Send copies the report and opens the GitHub new-issue form.

You can also file from [GitHub Issues](https://github.com/graycart/graycart-gb/issues). Do not paste org secrets into the repo or release binaries. See [CONTRIBUTING.md](CONTRIBUTING.md) for form fields and privacy notes.

## License

[MIT](LICENSE) — Copyright (c) 2026 Graycart.

The repository MIT license does **not** cover Blargg or Mooneye (or future Acid2) test ROM fixtures under `tests/fixtures/`. Those keep their upstream licenses; see [tests/fixtures/README.md](tests/fixtures/README.md) for provenance. Do not add commercial cartridges to git.
