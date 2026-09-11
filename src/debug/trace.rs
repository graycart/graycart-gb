//! Shared formatting for instruction traces and fault dumps.

use crate::cpu::{Cpu, Decoded};

/// Space-separated hex bytes (`"CB AE 0E"`).
pub fn format_bytes(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// One line for `--trace` / history: `PC: MNEMONIC | AF=...`.
pub fn format_trace_line(step: u64, pc: u16, decoded: &Decoded, cpu: &Cpu) -> String {
    format!(
        "{step}: {pc:04X}: {:<14} | {cpu}",
        decoded.instruction.mnemonic(pc)
    )
}

#[cfg(test)]
mod tests;
