# Changelog

## 0.11.1

Linux CI installs ALSA/udev headers so `cpal`/`gilrs` can build. Unix boot-ROM tests set directory modes with `PermissionsExt` instead of `set_readonly(false)` (clippy `-D warnings`).

## 0.11.0

Public GitHub cut: MIT license, contributor-facing README/AGENTS, architecture notes, and CI. Product behavior is the 0.10.12 CGB-ready emulator (Automatic = CGB silicon, host audio device policy, window-independent host tick).

Not 1.0.0: packaging, settings migration, save/state compatibility, and trusted cross-platform play are still open.
