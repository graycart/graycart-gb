# Blargg fixtures

From [retrio/gb-test-roms](https://github.com/retrio/gb-test-roms).

Targets:

- `cpu_instrs/` — CPU arithmetic / flags (green)
- `dmg_sound/` — APU conformance (Phase 8H)
- `cgb_sound/` — CGB APU (retrio `cgb_sound`; Fast Native CGB)

```bash
cargo test --test roms blargg_dmg_sound_matrix -- --ignored --nocapture
cargo test --test roms blargg_cgb_sound_matrix -- --ignored --nocapture
```
