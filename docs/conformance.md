# Tests and conformance

## `cargo test`

Rust unit tests (`src/**/tests.rs`) and small integration tests. **No ROM download.** This is what CI runs and what a clone must pass.

## ROM harnesses (temporary in-tree)

Until [`graycart-tests`](https://github.com/graycart/graycart-tests) owns Blargg / Mooneye / Acid2 via pinned fetch:

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

## Planned contract

| Command | Meaning |
|---------|---------|
| `cargo test` | Code-owned tests (clone-and-go) |
| `graycart-test run --suite …` | External suite (future); missing suite is not a unit-test failure |

Acid2 is not vendored here yet; it will live in `graycart-tests`.
