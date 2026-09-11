# Changelog

Prebuilt binaries for tagged releases ship on
[GitHub Releases](https://github.com/graycart/graycart-gb/releases).
Asset names follow `graycart-{version}-{target}` (Windows adds `.exe`) with a
matching `.sha256` sidecar. Targets: `x86_64-unknown-linux-gnu`,
`x86_64-pc-windows-msvc`, `aarch64-apple-darwin`, `x86_64-apple-darwin`.
Git tag `vX.Y.Z` must match `Cargo.toml` `[package].version` or the release
workflow fails.

## 0.11.2

Public hardening A–G (docs, templates, fixture provenance, CI, release workflow,
consent-gated crash/bug/feature filing, DRY/SRP splits) plus Windows/host
playability fixes from [#10](https://github.com/graycart/graycart-gb/issues/10) /
[#11](https://github.com/graycart/graycart-gb/pull/11):

- Audio: negotiate host channel counts instead of forcing stereo; probe common
  rates inside supported ranges; fall back across enumerated output devices when
  the preferred/default device fails.
- Clipboard: cross-platform report Copy via `arboard` with success/failure toast
  (replaces macOS-only `pbcopy`).
- Reporting: Send without a fine-grained PAT is a first-class path (copy body +
  open GitHub new-issue form); Help menu labels the token as optional.

## 0.11.1

Linux CI installs ALSA/udev headers so `cpal`/`gilrs` can build. Unix boot-ROM tests set directory modes with `PermissionsExt` instead of `set_readonly(false)` (clippy `-D warnings`).

## 0.11.0

Public GitHub cut: MIT license, contributor-facing README/AGENTS, architecture notes, and CI. Product behavior is the 0.10.12 CGB-ready emulator (Automatic = CGB silicon, host audio device policy, window-independent host tick).

Not 1.0.0: packaging, settings migration, save/state compatibility, and trusted cross-platform play are still open.
