# Agent guidelines — Graycart (`graycart-gb`)

How to change this emulator. Product name **Graycart**; crate/binary **`graycart`**. Family: [graycart](https://github.com/graycart/graycart), GBA later.

## Hardware-first

Commercial progress is a **smoke test**. Conformance ROMs are the **accuracy test**.

- Fix the **owning subsystem** (PPU, APU, timer, bus, boot, interrupts, DMA). Trace to the earliest wrong hardware-visible state.
- **No title hacks.** No Crystal/Kirby/FFA special cases. A game clearing OAM/CRAM is usually a real program path after an earlier bad observation.
- **Pan Docs first** for CGB and APU, then test-ROM expectations, then trusted traces. A task that cannot name its reference and acceptance test is not ready to implement.
- Unsupported mappers fail clearly (`unsupported mapper: MBC7`), never silent ROM-only.

[Pan Docs Audio](https://gbdev.io/pandocs/Audio.html) is authoritative for NR10–NR52. Do not invent APU behavior from commercial titles.

## Core vs host

The **lib** is the machine. The **binary** (`frontend/`) owns winit/wgpu/egui/cpal. Core speaks framebuffer, PCM, and `GameBoyButton` only. Headless `--frames` must not depend on the window stack.

- Shade → RGB and display effects live in `frontend/video` only.
- UI: no disabled “coming soon” items.
- Release builds are the supported play mode. `FramePacer` is the wall-clock cadence; do not treat debug slowness as an APU bug.

## Modules

If something has its own state, rules, tests, or lifecycle, it gets its own module. Keep orchestration thin (`execute.rs`, `bus/mod.rs`, `main.rs`). Do not dump PPU/timer/APU logic into the bus or CPU execute.

CPU → Bus → Cartridge → MBC; Timer / PPU / APU / Joypad / `hw/` (KEY1, clock, HDMA) stay distinct. VRAM banks in `ppu/vram.rs`, WRAM in `bus/wram.rs`, CRAM in `ppu/cram.rs`.

Tests: `src/<module>/tests.rs` via `#[cfg(test)] mod tests;` — not inline in production files. Integration under `tests/` uses the public API. ROM harnesses live under `tests/roms/` with fixtures in `tests/fixtures/`.

## Parallelism

Independent tasks: disjoint files, no shared unfinished types. Do not have two implementers on `app.rs` or the same unfinished API. Interface producers before consumers.

## SemVer

`Cargo.toml` `[package].version` is the only product version (window title, About, GCS1 `graycart_version`).

- Completed slices bump **patch** (`0.11.1`, …) unless the change is a real 0.x **minor**.
- **Do not** ship `1.0.0` until DMG and CGB are stable, installers exist, settings migrations are trusted, save/state compatibility is defined, and basic cross-platform play is trusted.
- Git tag **`vX.Y.Z` must equal** the crate version. CI release jobs fail on mismatch and attach `graycart-{version}-{target}` binaries.

## Validation (required)

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Never commit `carts/*.gb`, `.sav`, `.gcs*`, boot firmware, secrets, or skip hooks. Never force-push `main`.

Do not weaken tests (drop asserts, skip without a reason, title-specific expected values).
