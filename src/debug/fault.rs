//! Structured execution faults and printable crash reports.

use super::history::InstructionHistory;
use super::trace::format_bytes;
use crate::bus::Bus;
use crate::cpu::{Cpu, Instruction, StepError, interrupts, peek_bytes};
use crate::ppu::{LCDC, LY, STAT};
use crate::timer::{DIV, TAC, TIMA, TMA};

/// Classified reason execution stopped abnormally.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CpuFault {
    Decode {
        pc: u16,
        opcode: u8,
        bytes: [u8; 8],
    },
    Unimplemented {
        pc: u16,
        instruction: Instruction,
    },
    StuckHalt {
        waited: u64,
    },
    StepLimit {
        limit: u64,
    },
    /// LCD never produced VBlank within a step watchdog (headless runner).
    NoVBlank {
        steps_since_frame: u64,
    },
}

impl CpuFault {
    pub fn from_step_error(err: StepError) -> Self {
        match err {
            StepError::Decode { pc, bytes } => Self::Decode {
                pc,
                opcode: bytes[0],
                bytes,
            },
            StepError::Unimplemented { pc, instruction } => Self::Unimplemented { pc, instruction },
        }
    }

    pub fn reason_label(&self) -> &'static str {
        match self {
            Self::Decode { .. } => "decode failure",
            Self::Unimplemented { .. } => "unimplemented execution",
            Self::StuckHalt { .. } => "stuck HALT",
            Self::StepLimit { .. } => "step limit",
            Self::NoVBlank { .. } => "no VBlank",
        }
    }

    pub fn pc(&self) -> Option<u16> {
        match self {
            Self::Decode { pc, .. } | Self::Unimplemented { pc, .. } => Some(*pc),
            _ => None,
        }
    }
}

/// Successful termination vs structured fault.
#[derive(Debug, Clone)]
pub enum RunOutcome {
    /// Reached the requested frame count cleanly.
    FrameLimit {
        frames: u64,
        steps: u64,
    },
    Fault(Box<FaultReport>),
}

/// Self-contained crash dump for terminal / agent debugging.
#[derive(Debug, Clone)]
pub struct FaultReport {
    pub fault: CpuFault,
    pub pc: u16,
    pub rom_bank: u16,
    pub physical: Option<usize>,
    pub opcode_bytes: [u8; 8],
    pub frames: u64,
    pub steps: u64,
    pub af: u16,
    pub bc: u16,
    pub de: u16,
    pub hl: u16,
    pub sp: u16,
    pub z: bool,
    pub n: bool,
    pub h: bool,
    pub c: bool,
    pub ime: bool,
    pub halted: bool,
    pub ie: u8,
    pub if_: u8,
    pub pending: u8,
    pub ly: u8,
    pub lcdc: u8,
    pub stat: u8,
    pub mode: u8,
    pub div: u8,
    pub tima: u8,
    pub tma: u8,
    pub tac: u8,
    pub history: InstructionHistory,
    pub mem_before: [u8; 8],
    pub mem_at: [u8; 8],
    pub mem_after: [u8; 8],
}

impl FaultReport {
    pub fn capture(
        fault: CpuFault,
        cpu: &Cpu,
        bus: &Bus,
        history: &InstructionHistory,
        frames: u64,
        steps: u64,
    ) -> Self {
        let pc = fault.pc().unwrap_or(cpu.pc);
        let rom_bank = bus.cartridge.switchable_rom_bank();
        let physical = bus.cartridge.physical_rom_offset(pc);
        let opcode_bytes = match &fault {
            CpuFault::Decode { bytes, .. } => *bytes,
            _ => peek_bytes(bus, pc),
        };
        let mem_before = peek_bytes(bus, pc.wrapping_sub(0x10));
        let mem_at = peek_bytes(bus, pc);
        let mem_after = peek_bytes(bus, pc.wrapping_add(8));

        Self {
            fault,
            pc,
            rom_bank,
            physical,
            opcode_bytes,
            frames,
            steps,
            af: cpu.af(),
            bc: cpu.bc(),
            de: cpu.de(),
            hl: cpu.hl(),
            sp: cpu.sp,
            z: cpu.flag_z(),
            n: cpu.flag_n(),
            h: cpu.flag_h(),
            c: cpu.flag_c(),
            ime: cpu.ime,
            halted: cpu.halted,
            ie: bus.read8(interrupts::IE_ADDR),
            if_: bus.read8(interrupts::IF_ADDR),
            pending: interrupts::pending_mask(bus),
            ly: bus.read8(LY),
            lcdc: bus.read8(LCDC),
            stat: bus.read8(STAT),
            mode: bus.ppu.mode().as_u8(),
            div: bus.read8(DIV),
            tima: bus.read8(TIMA),
            tma: bus.read8(TMA),
            tac: bus.read8(TAC),
            history: history.clone(),
            mem_before,
            mem_at,
            mem_after,
        }
    }

    fn instruction_detail(&self) -> String {
        match &self.fault {
            CpuFault::Decode { opcode, pc, .. } => {
                format!("unable to decode opcode ${opcode:02X} at ${pc:04X}")
            }
            CpuFault::Unimplemented { instruction, pc } => {
                format!(
                    "decoded but unimplemented: {} at ${pc:04X}",
                    instruction.mnemonic(*pc)
                )
            }
            CpuFault::StuckHalt { waited } => {
                format!("HALT watchdog exceeded ({waited} steps without wake)")
            }
            CpuFault::StepLimit { limit } => {
                format!("execution exceeded configured step limit ({limit})")
            }
            CpuFault::NoVBlank { steps_since_frame } => {
                format!("no VBlank after {steps_since_frame} steps (LCD off or stuck?)")
            }
        }
    }
}

impl std::fmt::Display for FaultReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "=== CPU FAULT ====================================================="
        )?;
        writeln!(f)?;
        writeln!(f, "reason:        {}", self.fault.reason_label())?;
        writeln!(f, "pc:            ${:04X}", self.pc)?;
        writeln!(f, "rom bank:      ${:02X}", self.rom_bank)?;
        if let Some(phys) = self.physical {
            writeln!(f, "physical:      ${phys:06X}")?;
        }
        writeln!(f, "opcode bytes:  {}", format_bytes(&self.opcode_bytes))?;
        writeln!(f, "frame:         {}", fmt_u64(self.frames))?;
        writeln!(f, "steps:         {}", fmt_u64(self.steps))?;
        writeln!(f)?;
        writeln!(f, "instruction:")?;
        writeln!(f, "  {}", self.instruction_detail())?;
        writeln!(f)?;
        writeln!(f, "cpu:")?;
        writeln!(
            f,
            "  AF=${:04X}  BC=${:04X}  DE=${:04X}  HL=${:04X}",
            self.af, self.bc, self.de, self.hl
        )?;
        writeln!(f, "  SP=${:04X}  PC=${:04X}", self.sp, self.pc)?;
        writeln!(
            f,
            "  Z={} N={} H={} C={}",
            u8::from(self.z),
            u8::from(self.n),
            u8::from(self.h),
            u8::from(self.c)
        )?;
        writeln!(f, "  IME={}  halted={}", self.ime, self.halted)?;
        writeln!(f)?;
        writeln!(f, "interrupts:")?;
        writeln!(f, "  IE=${:02X}", self.ie)?;
        writeln!(f, "  IF=${:02X}", self.if_)?;
        writeln!(f, "  pending=${:02X}", self.pending)?;
        writeln!(f)?;
        writeln!(f, "ppu:")?;
        writeln!(f, "  LY=${:02X}", self.ly)?;
        writeln!(f, "  LCDC=${:02X}", self.lcdc)?;
        writeln!(f, "  STAT=${:02X}", self.stat)?;
        writeln!(f, "  mode={}", self.mode)?;
        writeln!(f, "  frame={}", fmt_u64(self.frames))?;
        writeln!(f)?;
        writeln!(f, "timer:")?;
        writeln!(f, "  DIV=${:02X}", self.div)?;
        writeln!(f, "  TIMA=${:02X}", self.tima)?;
        writeln!(f, "  TMA=${:02X}", self.tma)?;
        writeln!(f, "  TAC=${:02X}", self.tac)?;
        writeln!(f)?;
        writeln!(f, "recent instructions:")?;
        if self.history.is_empty() {
            writeln!(f, "  (none)")?;
        } else {
            for e in self.history.iter() {
                writeln!(f, "  {}", e.format_line())?;
            }
        }
        match &self.fault {
            CpuFault::Decode { opcode, .. } => {
                writeln!(
                    f,
                    "> {:04X}: {:02X}             <decode failure>",
                    self.pc, opcode
                )?;
            }
            CpuFault::Unimplemented { instruction, pc } => {
                writeln!(
                    f,
                    "> {:04X}: {:<14} <unimplemented>",
                    pc,
                    instruction.mnemonic(*pc)
                )?;
            }
            _ => {
                writeln!(f, "> {:04X}:                 <fault>", self.pc)?;
            }
        }
        writeln!(f)?;
        writeln!(f, "memory around PC:")?;
        writeln!(
            f,
            "  {:04X}: {}",
            self.pc.wrapping_sub(0x10),
            format_bytes(&self.mem_before)
        )?;
        writeln!(f, "  {:04X}: {}", self.pc, format_bytes(&self.mem_at))?;
        writeln!(
            f,
            "  {:04X}: {}",
            self.pc.wrapping_add(8),
            format_bytes(&self.mem_after)
        )?;
        writeln!(f)?;
        write!(
            f,
            "==================================================================="
        )?;
        Ok(())
    }
}

fn fmt_u64(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out.chars().rev().collect()
}

#[cfg(test)]
mod tests;
