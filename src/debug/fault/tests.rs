use super::*;
use crate::bus::Bus;
use crate::cpu::Cpu;

#[test]
fn classifies_decode_and_unimplemented() {
    let d = CpuFault::from_step_error(StepError::Decode {
        pc: 0x59D8,
        bytes: [0xCB, 0x20, 0, 0, 0, 0, 0, 0],
    });
    assert_eq!(d.reason_label(), "decode failure");
    assert_eq!(d.pc(), Some(0x59D8));

    let u = CpuFault::from_step_error(StepError::Unimplemented {
        pc: 0x0100,
        instruction: Instruction::Nop,
    });
    assert_eq!(u.reason_label(), "unimplemented execution");
}

#[test]
fn report_contains_bank_and_bytes() {
    let mut rom = vec![0x00; 0x8000];
    rom[0x100] = 0xCB;
    rom[0x101] = 0x20; // SLA B — not decoded yet
    let cpu = Cpu::after_boot();
    let bus = Bus::from_rom(rom);
    let history = InstructionHistory::new();
    let fault = CpuFault::Decode {
        pc: 0x0100,
        opcode: 0xCB,
        bytes: peek_bytes(&bus, 0x0100),
    };
    let report = FaultReport::capture(fault, &cpu, &bus, &history, 12, 3456);
    let text = report.to_string();
    assert!(text.contains("decode failure"));
    assert!(text.contains("pc:            $0100"));
    assert!(text.contains("rom bank:"));
    assert!(text.contains("CB 20"));
    assert!(text.contains("frame:         12"));
    assert!(text.contains("steps:         3,456"));
    assert!(text.contains("=== CPU FAULT"));
}
