# Contributing to Graycart (`graycart-gb`)

Thanks for helping improve **Graycart**. This repository is the DMG + CGB emulator in the [Graycart family](https://github.com/graycart/graycart). The crate and binary are named **`graycart`**.

For engineering norms (hardware-first, module ownership, SemVer), see [`AGENTS.md`](./AGENTS.md).

## Build and run

Rust stable (see CI). From this repo:

```bash
cargo run --release
cargo run --release -- path/to/game.gb
```

Headless smoke:

```bash
cargo run --release -- --frames 120 path/to/game.gb
```

Do not commit commercial ROMs, `.sav` / `.gcs*` files, or Nintendo boot firmware. Local dumps belong under `carts/` (gitignored) or stay outside the tree.

## Validation (required before a change is done)

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

`cargo test` runs Rust unit and integration tests only. Optional ROM matrices:

```bash
cargo test --test roms -- --ignored
```

Those ignored ROM suites are manual / opt-in and are not CI-blocking.

## Filing bugs and feature requests

Use GitHub issue forms under **New issue**:

| Form | Use for |
|------|---------|
| **Bug report** | Incorrect emulation, crashes, host/UI failures |
| **Feature request** | New capability or UX (title + description only) |

### Bug form field ids (for humans and Stream F)

Stable `id`s on `.github/ISSUE_TEMPLATE/bug.yml` — keep API / in-app body builders aligned:

| `id` | Meaning |
|------|---------|
| `version` | Graycart product version |
| `commit` | Git commit / SHA (or `unknown`) |
| `os` | Host OS + arch |
| `rom_title` | ROM header title (basename OK; no full paths) |
| `hardware_mode` | Automatic / Game Boy / Game Boy Color |
| `boot_mode` | Fast vs Boot ROM |
| `audio` | Audio backend / device label |
| `controller` | Keyboard / controller summary |
| `diagnostics` | F9 / panic / `FaultReport` text (or `none`) |
| `repro_mode` | Reproduces on Automatic / DMG / CGB |
| `summary` | Steps + expected vs actual |

Feature form body field: `description` only (issue **title** is the GitHub title field).

### Privacy

Never attach ROM bytes, saves, states, or boot firmware. Prefer header title + basename; scrub username-bearing paths.

### In-app filing (coming in 0.11.x)

Help → **Report bug…** / **Request feature…** and crash-consent filing will create issues via a **user fine-grained PAT** (settings and/or `GRAYCART_GITHUB_TOKEN`). Configure the token in the app when that lands — do not paste org secrets into the repo or release binaries. Until then, use the GitHub issue forms above.

## Pull requests

- One focused change per PR when practical.
- Run the validation triad above; the PR template checklist mirrors it.
- Prefer fixing the owning subsystem over title-specific special cases.
- Stay on **0.11.x** unless maintainers ask otherwise; do not chase 1.0 here.

## License

Code in this repository is MIT (see [`LICENSE`](./LICENSE)). Test fixtures under `tests/fixtures/` keep their **upstream** licenses and are **not** covered by the repo MIT grant — see provenance notes under `tests/fixtures/`.
