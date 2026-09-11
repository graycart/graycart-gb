use crate::cpu::decode;

/// Disassemble up to `count` instructions starting at `start` in `data`.
///
/// Stops early on an unknown opcode (prints a `DB` line for that byte).
pub fn disassemble(data: &[u8], start: usize, count: usize) {
    let mut pc = start;
    for _ in 0..count {
        if pc >= data.len() {
            break;
        }

        match decode(&data[pc..]) {
            Some(decoded) => {
                let end = pc + usize::from(decoded.len);
                let hex = format_hex(&data[pc..end]);
                let mnemonic = decoded.instruction.mnemonic(pc as u16);
                println!("{pc:04X}: {hex:<11} {mnemonic}");
                pc = end;
            }
            None => {
                let byte = data[pc];
                println!("{pc:04X}: {byte:02X}          DB ${byte:02X}");
                break;
            }
        }
    }
}

fn format_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}
