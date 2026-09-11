//! CPU-side interrupt pending / wake / service rules.
//!
//! IE (`$FFFF`) and IF (`$FF0F`) storage live on the bus; this module decides
//! what the CPU does with them.
//!
//! [Pan Docs — Interrupts](https://gbdev.io/pandocs/Interrupts.html)

use super::Cpu;
use crate::bus::Bus;

pub const IF_ADDR: u16 = 0xFF0F;
pub const IE_ADDR: u16 = 0xFFFF;
const INTERRUPT_MASK: u8 = 0x1F;

/// Interrupt sources (priority = enum order / lowest bit first).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interrupt {
    VBlank,
    LcdStat,
    Timer,
    Serial,
    Joypad,
}

impl Interrupt {
    pub fn bit(self) -> u8 {
        match self {
            Self::VBlank => 0,
            Self::LcdStat => 1,
            Self::Timer => 2,
            Self::Serial => 3,
            Self::Joypad => 4,
        }
    }

    pub fn mask(self) -> u8 {
        1 << self.bit()
    }

    pub fn vector(self) -> u16 {
        match self {
            Self::VBlank => 0x0040,
            Self::LcdStat => 0x0048,
            Self::Timer => 0x0050,
            Self::Serial => 0x0058,
            Self::Joypad => 0x0060,
        }
    }

    fn from_bit(bit: u8) -> Option<Self> {
        match bit {
            0 => Some(Self::VBlank),
            1 => Some(Self::LcdStat),
            2 => Some(Self::Timer),
            3 => Some(Self::Serial),
            4 => Some(Self::Joypad),
            _ => None,
        }
    }
}

/// `IE & IF` bits that are both set (lower five only).
pub fn pending_mask(bus: &Bus) -> u8 {
    bus.read8(IE_ADDR) & bus.read8(IF_ADDR) & INTERRUPT_MASK
}

pub fn has_pending(bus: &Bus) -> bool {
    pending_mask(bus) != 0
}

/// Highest-priority pending interrupt, if any.
pub fn highest_pending(bus: &Bus) -> Option<Interrupt> {
    let pending = pending_mask(bus);
    (0..5).find_map(|bit| {
        if pending & (1 << bit) != 0 {
            Interrupt::from_bit(bit)
        } else {
            None
        }
    })
}

/// Service one interrupt: IME off, push PC, then ack + jump.
///
/// High-byte push happens first. If that write hits `IE` (`SP` was `$0000`),
/// `IE & IF` is re-sampled before the low-byte push — the interrupt actually
/// taken (or `$0000` if none remain) is chosen then. A low-byte write to `IE`
/// (`SP` was `$0001`) is too late to cancel.
pub fn service(cpu: &mut Cpu, bus: &mut Bus, _interrupt: Interrupt) {
    cpu.ime = false;
    cpu.ime_enable_pending = false;
    cpu.ime_enable_armed = false;
    cpu.halted = false;

    let [lo, hi] = cpu.pc.to_le_bytes();

    cpu.sp = cpu.sp.wrapping_sub(1);
    bus.write8(cpu.sp, hi);

    // Sample after high-byte push (ares / mooneye ie_push behavior).
    let selected = highest_pending(bus);

    cpu.sp = cpu.sp.wrapping_sub(1);
    bus.write8(cpu.sp, lo);

    match selected {
        Some(irq) => {
            let if_reg = bus.read8(IF_ADDR);
            bus.write8(IF_ADDR, if_reg & !irq.mask());
            cpu.pc = irq.vector();
        }
        None => {
            // Cancelled: IF unchanged, PC forced to $0000.
            cpu.pc = 0;
        }
    }
}

/// Called at the start of each CPU step (wake / service only).
///
/// Returns `true` if an interrupt was serviced (no opcode should be fetched
/// this step).
pub fn poll(cpu: &mut Cpu, bus: &mut Bus) -> bool {
    let pending = has_pending(bus);

    if cpu.halted {
        if !pending {
            return false;
        }
        // Wake on any pending interrupt; service only if IME is set.
        cpu.halted = false;
        if cpu.ime
            && let Some(irq) = highest_pending(bus)
        {
            service(cpu, bus, irq);
            return true;
        }
        return false;
    }

    if cpu.ime
        && let Some(irq) = highest_pending(bus)
    {
        service(cpu, bus, irq);
        return true;
    }

    false
}

/// Apply delayed `EI` after an instruction finishes.
///
/// `EI` sets `ime_enable_pending`. At the end of that same instruction it becomes
/// `ime_enable_armed`. At the end of the *following* instruction, `IME` turns on.
pub fn apply_delayed_ei(cpu: &mut Cpu) {
    if cpu.ime_enable_armed {
        cpu.ime = true;
        cpu.ime_enable_armed = false;
    }
    if cpu.ime_enable_pending {
        cpu.ime_enable_armed = true;
        cpu.ime_enable_pending = false;
    }
}

#[cfg(test)]
mod tests;
