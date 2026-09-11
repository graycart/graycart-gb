//! Explicit frozen emulator snapshot (`MachineStateV1`) — distinct from battery `save/`.

mod format;

#[cfg(test)]
mod tests;

pub use format::{
    Gcs1Error, Gcs1Header, Gcs1Metadata, decode_gcs1, encode_gcs1, rom_sha256, sanitize_title,
};

pub const SNAPSHOT_FORMAT_VERSION: u16 = 1;

use serde::{Deserialize, Serialize};

/// LR35902 register file + interrupt control (no `ExecSession`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CpuStateV1 {
    pub a: u8,
    pub f: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,
    pub sp: u16,
    pub pc: u16,
    pub ime: bool,
    pub ime_enable_pending: bool,
    pub ime_enable_armed: bool,
    pub halted: bool,
}

/// WRAM, HRAM, I/O stub bytes, IE, serial capture, OAM-DMA suppress.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BusRamStateV1 {
    #[serde(with = "serde_bytes")]
    pub vram: [u8; 0x2000],
    #[serde(with = "serde_bytes")]
    pub wram: [u8; 0x2000],
    #[serde(with = "serde_bytes")]
    pub io: [u8; 0x80],
    #[serde(with = "serde_bytes")]
    pub hram: [u8; 0x7F],
    pub ie: u8,
    pub serial: Vec<u8>,
    pub oam_dma_suppress_t: u32,
}

/// Mapper banking + SRAM + MBC3 RTC runtime (ROM bytes excluded).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CartStateV1 {
    pub ram: Vec<u8>,
    pub mapper: MapperStateV1,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MapperStateV1 {
    None,
    Mbc1(Mbc1StateV1),
    Mbc2(Mbc2StateV1),
    Mbc3(Mbc3StateV1),
    Mbc5(Mbc5StateV1),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mbc1StateV1 {
    pub rom_bank: u8,
    pub ram_bank: u8,
    pub ram_enable: bool,
    pub mode: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mbc2StateV1 {
    pub rom_bank: u8,
    pub ram_enable: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mbc3StateV1 {
    pub rom_bank: u8,
    pub ram_bank: u8,
    pub ram_enable: bool,
    pub rtc_reg: u8,
    pub rtc: RtcStateV1,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RtcStateV1 {
    pub s: u8,
    pub m: u8,
    pub h: u8,
    pub dl: u8,
    pub dh: u8,
    pub latched: [u8; 5],
    pub latch_prev: u8,
    pub cycle_accum: u64,
    pub unix_secs: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mbc5StateV1 {
    pub rom_bank_low: u8,
    pub rom_bank_high: u8,
    pub ram_bank: u8,
    pub ram_enable: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimerStateV1 {
    pub div: u16,
    pub tima: u8,
    pub tma: u8,
    pub tac: u8,
    pub reload_delay: u8,
}

/// PPU sans `framebuffer` — captures at VBlank/frame boundary only.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PpuStateV1 {
    pub lcdc: u8,
    pub stat: u8,
    pub scy: u8,
    pub scx: u8,
    pub ly: u8,
    pub lyc: u8,
    pub dma: u8,
    pub bgp: u8,
    pub obp0: u8,
    pub obp1: u8,
    pub wy: u8,
    pub wx: u8,
    pub mode: u8,
    pub dot: u16,
    pub window_line: u8,
    pub window_y_active: bool,
    #[serde(with = "serde_bytes")]
    pub oam: [u8; 0xA0],
    pub dma_active: bool,
    pub dma_src: u16,
    pub dma_index: u8,
    pub dma_delay: u8,
}

/// APU channel internals + mixer — excludes `stats`, `samples`, `cycles_per_sample`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApuStateV1 {
    pub powered: bool,
    pub frame_seq_step: u8,
    pub ch1: Ch1StateV1,
    pub ch2: SquareStateV1,
    pub ch3: WaveStateV1,
    pub ch4: NoiseStateV1,
    pub nr50: u8,
    pub nr51: u8,
    pub wave_ram: [u8; 16],
    pub sample_phase: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ch1StateV1 {
    pub square: SquareStateV1,
    pub sweep_period: u8,
    pub sweep_negate: bool,
    pub sweep_shift: u8,
    pub sweep_timer: u8,
    pub sweep_enable: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SquareStateV1 {
    pub enabled: bool,
    pub dac_on: bool,
    pub length: u8,
    pub length_enable: bool,
    pub duty: u8,
    pub envelope_volume: u8,
    pub envelope_dir: bool,
    pub envelope_period: u8,
    pub envelope_timer: u8,
    pub frequency: u16,
    pub frequency_timer: u16,
    pub duty_pos: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WaveStateV1 {
    pub dac_on: bool,
    pub length: u8,
    pub length_enable: bool,
    pub volume_code: u8,
    pub frequency: u16,
    pub frequency_timer: u16,
    pub pos_nib: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoiseStateV1 {
    pub enabled: bool,
    pub dac_on: bool,
    pub length: u8,
    pub length_enable: bool,
    pub envelope_volume: u8,
    pub envelope_dir: bool,
    pub envelope_period: u8,
    pub envelope_timer: u8,
    pub clock_shift: u8,
    pub width_mode: bool,
    pub divisor_code: u8,
    pub lfsr: u16,
    pub timer: u16,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JoypadStateV1 {
    pub select: u8,
    pub pressed: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeStateV1 {
    pub post_boot: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MachineStateV1 {
    pub cpu: CpuStateV1,
    pub bus: BusRamStateV1,
    pub cart: CartStateV1,
    pub timer: TimerStateV1,
    pub ppu: PpuStateV1,
    pub apu: ApuStateV1,
    pub joypad: JoypadStateV1,
    pub runtime: RuntimeStateV1,
}

impl MachineStateV1 {
    /// Approximate heap footprint for rewind budgeting (Vec + serial only).
    pub fn heap_bytes(&self) -> usize {
        self.bus.serial.len() + self.cart.ram.len() + std::mem::size_of::<Self>()
    }
}

/// Capture live emulator state into a frozen snapshot (Task 2).
pub fn capture(cpu: &crate::Cpu, bus: &crate::Bus) -> MachineStateV1 {
    MachineStateV1 {
        cpu: cpu.snapshot(),
        bus: bus.snapshot_ram(),
        cart: CartStateV1 {
            ram: bus.cartridge.snapshot_ram(),
            mapper: bus.cartridge.snapshot_mapper(),
        },
        timer: bus.timer.snapshot(),
        ppu: bus.ppu.snapshot(),
        apu: bus.apu.snapshot(),
        joypad: bus.joypad.snapshot(),
        runtime: RuntimeStateV1 { post_boot: true },
    }
}

/// Apply a frozen snapshot onto live emulator state (Task 2).
pub fn restore(state: &MachineStateV1, cpu: &mut crate::Cpu, bus: &mut crate::Bus) {
    cpu.apply_snapshot(&state.cpu);
    bus.apply_snapshot_ram(&state.bus);
    bus.cartridge.apply_ram(&state.cart.ram);
    bus.cartridge.apply_mapper(&state.cart.mapper);
    bus.cartridge.mark_save_dirty();
    bus.timer.apply_snapshot(&state.timer);
    bus.ppu.apply_snapshot(&state.ppu);
    bus.apu.apply_snapshot(&state.apu);
    bus.joypad.apply_snapshot(&state.joypad);
}
