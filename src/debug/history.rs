//! Fixed-size rolling buffer of recently executed instructions.

use crate::bus::Bus;
use crate::cpu::{Cpu, Instruction};

/// Entries retained for fault reports.
pub const HISTORY_LEN: usize = 32;

/// Snapshot of one successfully fetched instruction (pre-execute registers).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntry {
    pub pc: u16,
    pub rom_bank: u16,
    pub bytes: [u8; 3],
    pub mnemonic: String,
    pub af: u16,
    pub bc: u16,
    pub de: u16,
    pub hl: u16,
    pub sp: u16,
    pub ime: bool,
}

impl HistoryEntry {
    pub fn capture(cpu: &Cpu, bus: &Bus, bytes: [u8; 3], instruction: Instruction) -> Self {
        let pc = cpu.pc;
        Self {
            pc,
            rom_bank: bus.cartridge.switchable_rom_bank(),
            bytes,
            mnemonic: instruction.mnemonic(pc),
            af: cpu.af(),
            bc: cpu.bc(),
            de: cpu.de(),
            hl: cpu.hl(),
            sp: cpu.sp,
            ime: cpu.ime,
        }
    }

    pub fn format_line(&self) -> String {
        format!(
            "{:04X}: {:<14} bank={:02X} AF={:04X} BC={:04X} DE={:04X} HL={:04X} SP={:04X} IME={}",
            self.pc,
            self.mnemonic,
            self.rom_bank,
            self.af,
            self.bc,
            self.de,
            self.hl,
            self.sp,
            self.ime
        )
    }
}

/// Ring buffer of the last [`HISTORY_LEN`] fetched instructions.
#[derive(Debug, Clone, Default)]
pub struct InstructionHistory {
    entries: Vec<HistoryEntry>,
}

impl InstructionHistory {
    pub fn new() -> Self {
        Self {
            entries: Vec::with_capacity(HISTORY_LEN),
        }
    }

    pub fn push(&mut self, entry: HistoryEntry) {
        if self.entries.len() == HISTORY_LEN {
            self.entries.remove(0);
        }
        self.entries.push(entry);
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &HistoryEntry> {
        self.entries.iter()
    }
}

#[cfg(test)]
mod tests;
