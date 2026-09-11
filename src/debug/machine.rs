//! Immutable machine introspection for the Debug Monitor (observation only).

use crate::bus::Bus;
use crate::cpu::{Cpu, FLAG_C, FLAG_H, FLAG_N, FLAG_Z, decode, peek_bytes};
use crate::debug::ExecSession;
use crate::input::GameBoyButton;
use crate::ppu::MODE2_DOTS;
use crate::timer::{DIV, TAC, TIMA, TMA};

/// Full core snapshot — no host/window/audio device state.
#[derive(Debug, Clone)]
pub struct MachineDebug {
    pub cpu: CpuDebug,
    pub ppu: PpuDebug,
    pub apu: ApuDebug,
    pub timer: TimerDebug,
    pub interrupts: InterruptDebug,
    pub input: InputDebug,
    pub session_steps: u64,
    pub session_frames: u64,
}

impl MachineDebug {
    pub fn capture(cpu: &Cpu, bus: &Bus, session: &ExecSession) -> Self {
        let bytes = peek_bytes(bus, cpu.pc);
        let opcode_bytes = [bytes[0], bytes[1], bytes[2]];
        let mnemonic = match decode(&bytes) {
            Some(d) => d.instruction.mnemonic(cpu.pc),
            None => "???".to_string(),
        };
        Self {
            cpu: CpuDebug::from_cpu(cpu, opcode_bytes, mnemonic),
            ppu: PpuDebug::from_ppu(&bus.ppu, session.frames),
            apu: bus.apu.debug_snapshot(),
            timer: TimerDebug::from_timer(&bus.timer),
            interrupts: InterruptDebug {
                ie: bus.read8(0xFFFF),
                if_: bus.read8(0xFF0F),
            },
            input: InputDebug::from_joypad(&bus.joypad),
            session_steps: session.steps,
            session_frames: session.frames,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CpuDebug {
    pub a: u8,
    pub f: u8,
    pub af: u16,
    pub bc: u16,
    pub de: u16,
    pub hl: u16,
    pub sp: u16,
    pub pc: u16,
    pub z: bool,
    pub n: bool,
    pub h: bool,
    pub c: bool,
    pub ime: bool,
    pub halted: bool,
    pub opcode_bytes: [u8; 3],
    pub mnemonic: String,
}

impl CpuDebug {
    fn from_cpu(cpu: &Cpu, bytes: [u8; 3], mnemonic: String) -> Self {
        let f = cpu.f();
        Self {
            a: cpu.a,
            f,
            af: cpu.af(),
            bc: cpu.bc(),
            de: cpu.de(),
            hl: cpu.hl(),
            sp: cpu.sp,
            pc: cpu.pc,
            z: f & FLAG_Z != 0,
            n: f & FLAG_N != 0,
            h: f & FLAG_H != 0,
            c: f & FLAG_C != 0,
            ime: cpu.ime,
            halted: cpu.halted,
            opcode_bytes: bytes,
            mnemonic,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PpuDebug {
    pub ly: u8,
    pub mode: u8,
    pub line_cycles: u32,
    pub lcdc: u8,
    pub stat: u8,
    pub lcd_enabled: bool,
    pub mode0_start: u32,
    pub mode3_len: u32,
    pub frame_index: u64,
}

impl PpuDebug {
    fn from_ppu(ppu: &crate::ppu::Ppu, frame_index: u64) -> Self {
        Self {
            ly: ppu.ly(),
            mode: ppu.mode().as_u8(),
            line_cycles: ppu.line_cycles(),
            lcdc: ppu.lcdc(),
            stat: ppu.stat(),
            lcd_enabled: ppu.lcd_enabled(),
            mode0_start: ppu.mode0_start(),
            mode3_len: ppu.mode0_start().saturating_sub(MODE2_DOTS),
            frame_index,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ChannelDebug {
    pub active: bool,
    pub dac: bool,
    pub volume: u8,
    pub frequency: u16,
    pub duty: Option<u8>,
}

#[derive(Debug, Clone, Copy)]
pub struct ApuDebug {
    pub powered: bool,
    pub nr50: u8,
    pub nr51: u8,
    pub nr52: u8,
    pub ch1: ChannelDebug,
    pub ch2: ChannelDebug,
    pub ch3: ChannelDebug,
    pub ch4: ChannelDebug,
    pub peak_min: f32,
    pub peak_max: f32,
    pub samples_generated: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct TimerDebug {
    pub div: u8,
    pub tima: u8,
    pub tma: u8,
    pub tac: u8,
}

impl TimerDebug {
    fn from_timer(timer: &crate::timer::Timer) -> Self {
        Self {
            div: timer.read(DIV).unwrap_or(0),
            tima: timer.read(TIMA).unwrap_or(0),
            tma: timer.read(TMA).unwrap_or(0),
            tac: timer.read(TAC).unwrap_or(0),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct InterruptDebug {
    pub ie: u8,
    pub if_: u8,
}

#[derive(Debug, Clone, Copy)]
pub struct InputDebug {
    pub p1: u8,
    pub pressed_mask: u8,
}

impl InputDebug {
    fn from_joypad(joypad: &crate::input::Joypad) -> Self {
        let mut pressed_mask = 0u8;
        for (i, button) in GameBoyButton::ALL.iter().enumerate() {
            if joypad.is_pressed(*button) {
                pressed_mask |= 1 << i;
            }
        }
        Self {
            p1: joypad.read(),
            pressed_mask,
        }
    }

    pub fn is_pressed(self, button: GameBoyButton) -> bool {
        let i = GameBoyButton::ALL
            .iter()
            .position(|&b| b == button)
            .unwrap_or(0);
        self.pressed_mask & (1 << i) != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cpu::Cpu;

    #[test]
    fn capture_post_boot_cpu_fields() {
        let mut cpu = Cpu::after_boot();
        let mut bus = Bus::from_rom(vec![0x00; 0x8000]);
        crate::boot::apply_fast(&mut cpu, &mut bus);
        let session = ExecSession::new();
        let snap = MachineDebug::capture(&cpu, &bus, &session);
        assert_eq!(snap.cpu.pc, 0x0100);
        assert_eq!(snap.cpu.af, 0x01B0);
        assert!(!snap.cpu.halted);
        assert_eq!(snap.ppu.lcdc, 0x91);
        assert!(snap.apu.powered || !snap.apu.powered); // snapshot builds
        assert_eq!(snap.session_frames, 0);
    }
}
