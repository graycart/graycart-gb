//! Thin execution session: step counting, frame counting, instruction history.

use super::fault::{CpuFault, FaultReport, RunOutcome};
use super::history::{HistoryEntry, InstructionHistory};
use crate::bus::Bus;
use crate::cpu::{Cpu, Decoded, StepError, peek_bytes, step};

/// Tracks diagnostics across a run without owning the CPU/bus.
#[derive(Debug, Default)]
pub struct ExecSession {
    pub history: InstructionHistory,
    pub steps: u64,
    pub frames: u64,
}

impl ExecSession {
    pub fn new() -> Self {
        Self::default()
    }

    /// Step once; on success, records fetched instructions (`len > 0`) into history.
    pub fn step(&mut self, cpu: &mut Cpu, bus: &mut Bus) -> Result<Decoded, StepError> {
        let fetch_pc = cpu.pc;
        let pre_bytes = peek_bytes(bus, fetch_pc);
        let pre_af = cpu.af();
        let pre_bc = cpu.bc();
        let pre_de = cpu.de();
        let pre_hl = cpu.hl();
        let pre_sp = cpu.sp;
        let pre_ime = cpu.ime;
        let pre_bank = bus.cartridge.switchable_rom_bank();

        match step(cpu, bus) {
            Ok(decoded) => {
                self.steps = self.steps.saturating_add(1);
                if decoded.len > 0 {
                    self.history.push(HistoryEntry {
                        pc: fetch_pc,
                        rom_bank: pre_bank,
                        bytes: [pre_bytes[0], pre_bytes[1], pre_bytes[2]],
                        mnemonic: decoded.instruction.mnemonic(fetch_pc),
                        af: pre_af,
                        bc: pre_bc,
                        de: pre_de,
                        hl: pre_hl,
                        sp: pre_sp,
                        ime: pre_ime,
                    });
                }
                Ok(decoded)
            }
            Err(e) => {
                self.steps = self.steps.saturating_add(1);
                Err(e)
            }
        }
    }

    pub fn note_frame(&mut self) {
        self.frames = self.frames.saturating_add(1);
    }

    pub fn fault_report(&self, fault: CpuFault, cpu: &Cpu, bus: &Bus) -> FaultReport {
        FaultReport::capture(fault, cpu, bus, &self.history, self.frames, self.steps)
    }

    pub fn fault_from_step(&self, err: StepError, cpu: &Cpu, bus: &Bus) -> FaultReport {
        self.fault_report(CpuFault::from_step_error(err), cpu, bus)
    }

    /// Headless: run until `target` VBlank frames.
    pub fn run_frames(&mut self, cpu: &mut Cpu, bus: &mut Bus, target: u64) -> RunOutcome {
        let mut steps_since_frame = 0u64;
        while self.frames < target {
            match self.step(cpu, bus) {
                Ok(_) => {}
                Err(e) => {
                    return RunOutcome::Fault(Box::new(self.fault_from_step(e, cpu, bus)));
                }
            }
            steps_since_frame += 1;
            if bus.ppu.take_frame_ready() {
                self.note_frame();
                steps_since_frame = 0;
                continue;
            }
            if steps_since_frame > 500_000 {
                return RunOutcome::Fault(Box::new(self.fault_report(
                    CpuFault::NoVBlank { steps_since_frame },
                    cpu,
                    bus,
                )));
            }
        }
        RunOutcome::FrameLimit {
            frames: self.frames,
            steps: self.steps,
        }
    }
}

#[cfg(test)]
mod tests;
