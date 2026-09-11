pub mod apu;
pub mod boot;
pub mod bus;
pub mod cart;
pub mod cpu;
pub mod debug;
pub mod disasm;
pub mod hex;
pub mod hw;
pub mod input;
pub mod ppu;
pub mod save;
pub mod snapshot;
pub mod timer;

pub use apu::{Apu, Ch1Debug, HOST_SAMPLE_RATE, StereoSample};
pub use boot::{BootMode, apply_fast, prepare_boot, run as run_boot};
pub use bus::Bus;
pub use cart::{
    CartType, Cartridge, CartridgeCgbSupport, CgbFlag, Destination, Header, HeaderError,
    NINTENDO_LOGO, OLD_LICENSEE_USE_NEW, RamSize, RomSize, SGB_FLAG_ENABLED,
};
pub use cpu::{
    Cond, Cpu, Decoded, FLAG_C, FLAG_H, FLAG_N, FLAG_Z, Instruction, Interrupt, Operand8, Reg8,
    Reg16, StepError, decode, jr_target, peek_bytes, step,
};
pub use debug::{
    ApuDebug, ChannelDebug, CpuDebug, CpuFault, ExecSession, FaultReport, InputDebug,
    InterruptDebug, MachineDebug, PpuDebug, RunOutcome, TickProfile, TimerDebug, format_trace_line,
};
pub use disasm::disassemble;
pub use hex::hex_dump;
pub use hw::{
    CGB_SILICON_READY, CgbExecutionMode, CgbSupportClass, HardwareModel, HostHardwarePref,
    LaunchError, bus_from_cartridge, resolve_launch,
};
pub use input::{GameBoyButton, Joypad, P1};
pub use ppu::{
    Framebuffer, Ppu, SCREEN_HEIGHT, SCREEN_WIDTH, Shade, decode_tile_row, shade_from_bgp,
};
pub use save::{
    default_save_path, flush as flush_save, graycart_saves_dir, legacy_sidecar_save_path,
    load as load_save, load_with_fallback as load_save_with_fallback, save_path_in_dir,
};
pub use snapshot::{
    Gcs1Error, Gcs1Header, Gcs1Metadata, MachineStateV1, SNAPSHOT_FORMAT_VERSION, capture,
    decode_gcs1, encode_gcs1, restore, rom_sha256, sanitize_title,
};
pub use timer::Timer;
