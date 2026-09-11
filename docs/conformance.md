# Tests and conformance

## `cargo test`

Rust unit tests (`src/**/tests.rs`) and small integration tests. **No ROM download.** This is what CI runs and what a clone must pass.

## ROM harnesses

Blargg / Mooneye (and later Acid2) are in-tree fixtures plus ignored ROM harnesses:

```bash
cargo test --test roms blargg_cpu_instrs_matrix -- --ignored --nocapture
cargo test --test roms blargg_dmg_sound_matrix -- --ignored --nocapture
cargo test --test roms blargg_cgb_sound_matrix -- --ignored --nocapture
cargo test --test roms mooneye_acceptance_matrix -- --ignored --nocapture
```

Fixtures today: `tests/fixtures/blargg/`, `tests/fixtures/mooneye/`. Outcomes: `PASS` / `FAIL` / `TIMEOUT` / `UNSUPPORTED`.

Commercial dumps are **never** in git. Optional smoke:

```bash
cargo test --test roms commercial_rom_smoke_matrix -- --ignored --nocapture
```

Missing `carts/*.gb` (or `GRAYCART_CARTS`) skips those titles.

Acid2 is not vendored yet.
