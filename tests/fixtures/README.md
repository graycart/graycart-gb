# Test ROM fixtures

Commercial carts stay under `carts/` (local only, gitignored). Conformance ROMs live here.

## Licensing / provenance

**These fixtures are not covered by this repository’s MIT license.**

Graycart’s root [`LICENSE`](../../LICENSE) applies to Graycart source and docs only. Vendored Blargg and Mooneye test ROMs keep their **upstream** copyright and license terms. See:

| Suite | Path | Provenance |
|-------|------|------------|
| Blargg | [`blargg/`](blargg/) | Shay Green (“Blargg”); mirror [retrio/gb-test-roms](https://github.com/retrio/gb-test-roms). Notes: [`blargg/README.md`](blargg/README.md), [`blargg/LICENSE`](blargg/LICENSE). |
| Mooneye Test Suite | [`mooneye/`](mooneye/) | Joonas Javanainen (MIT). Prebuilts from [gekkio.fi](https://gekkio.fi/files/mooneye-test-suite/); source [Gekkio/mooneye-test-suite](https://github.com/Gekkio/mooneye-test-suite). Notes: [`mooneye/README.md`](mooneye/README.md), [`mooneye/LICENSE`](mooneye/LICENSE). |

Acid2 (and similar) ROMs are **not** vendored here yet; if added later they likewise keep upstream terms and are not Graycart MIT.

Do not add commercial cartridges under this tree.

## Layout

```text
tests/fixtures/
├── blargg/          # from https://github.com/retrio/gb-test-roms
│   ├── cpu_instrs/
│   ├── instr_timing/
│   ├── mem_timing/
│   ├── mem_timing-2/
│   ├── interrupt_time/
│   ├── oam_bug/
│   └── halt_bug.gb
└── mooneye/         # from https://gekkio.fi/files/mooneye-test-suite/
    └── acceptance/
        ├── timer/
        ├── oam_dma/
        ├── bits/
        └── …
```

```bash
# Blargg cpu_instrs (first correctness target)
cargo test --test roms blargg_cpu_instrs_matrix -- --ignored --nocapture

# Mooneye selective acceptance
cargo test --test roms mooneye_acceptance_matrix -- --ignored --nocapture
```

Outcomes: `PASS` / `FAIL` / `TIMEOUT` / `UNSUPPORTED` (missing opcode / load error).
