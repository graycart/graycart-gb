# Test ROM fixtures

Commercial carts stay under `carts/` (local only, gitignored). Conformance ROMs live here.

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
