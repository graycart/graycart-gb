# Architecture

Graycart is a **DMG + CGB** emulator split into a **library** (the machine) and a **binary** (the host).

```text
CPU → Bus → Cartridge (MBC0/1/2/3/5)
        ↘ hw/     HardwareModel, KEY1, ClockState, HDMA
        ↘ timer    DIV/TIMA/TMA/TAC
        ↘ ppu     LCD, palettes, CGB attrs/priority
        ↘ apu     NR10–NR52 (host device IDs stay in frontend/)
        ↘ input   GameBoyButton / FF00 only
save/     .sav (+ MBC3 RTC trailer)
snapshot/ GCS1 machine state
frontend/ winit, GPU present, egui, CPAL, gilrs
```

`$0143` (`CartridgeCgbSupport`) is not a hardware-model switch. `HardwareModel::Dmg` stays DMG-accurate; Automatic launch uses CGB silicon when `CGB_SILICON_READY`.

VRAM bank state is `ppu/vram.rs` (VBK `$FF4F`). WRAM banks are `bus/wram.rs` (SVBK `$FF70`). CGB palette RAM is `ppu/cram.rs`. Bus only routes those ports.

The frontend must not change PPU framebuffer semantics for cosmetics. Host audio selection: user device → OS default → explicit fallback (not persisted) → offline.
