use super::*;
use crate::cpu::Cpu;

#[test]
fn history_records_fetched_instructions() {
    let mut rom = vec![0x00; 0x200];
    rom[0x100] = 0x00; // NOP
    rom[0x101] = 0x00; // NOP
    let mut cpu = Cpu::after_boot();
    let mut bus = Bus::from_rom(rom);
    let mut session = ExecSession::new();
    session.step(&mut cpu, &mut bus).unwrap();
    session.step(&mut cpu, &mut bus).unwrap();
    assert_eq!(session.history.len(), 2);
    assert_eq!(session.history.iter().next().unwrap().pc, 0x0100);
    assert_eq!(session.steps, 2);
}
