use super::*;
use crate::bus::Bus;
use crate::cpu::Cpu;

#[test]
fn timer_interrupt_services_from_halt() {
    let mut cpu = Cpu::after_boot();
    cpu.pc = 0x1234;
    cpu.sp = 0xFFFE;
    cpu.halted = true;
    cpu.ime = true;

    let mut bus = Bus::from_rom(vec![0; 0x200]);
    bus.write8(IE_ADDR, Interrupt::Timer.mask());
    bus.write8(IF_ADDR, Interrupt::Timer.mask());

    assert!(poll(&mut cpu, &mut bus));

    assert!(!cpu.halted);
    assert!(!cpu.ime);
    assert_eq!(cpu.pc, 0x0050);
    assert_eq!(cpu.sp, 0xFFFC);
    assert_eq!(bus.read8(cpu.sp), 0x34);
    assert_eq!(bus.read8(cpu.sp.wrapping_add(1)), 0x12);
    assert_eq!(bus.read8(IF_ADDR) & Interrupt::Timer.mask(), 0);
}

#[test]
fn pending_without_ime_wakes_halt_without_service() {
    let mut cpu = Cpu::after_boot();
    cpu.pc = 0x2000;
    cpu.halted = true;
    cpu.ime = false;

    let mut bus = Bus::from_rom(vec![0; 0x200]);
    bus.write8(IE_ADDR, Interrupt::VBlank.mask());
    bus.write8(IF_ADDR, Interrupt::VBlank.mask());

    assert!(!poll(&mut cpu, &mut bus));
    assert!(!cpu.halted);
    assert_eq!(cpu.pc, 0x2000);
    assert_eq!(
        bus.read8(IF_ADDR) & Interrupt::VBlank.mask(),
        Interrupt::VBlank.mask()
    );
}

#[test]
fn priority_prefers_vblank_over_timer() {
    let mut bus = Bus::from_rom(vec![0; 0x200]);
    bus.write8(IE_ADDR, 0x1F);
    bus.write8(IF_ADDR, Interrupt::VBlank.mask() | Interrupt::Timer.mask());
    assert_eq!(highest_pending(&bus), Some(Interrupt::VBlank));
}

#[test]
fn ie_push_high_byte_can_cancel_to_pc_zero() {
    let mut cpu = Cpu::after_boot();
    cpu.pc = 0x0200; // high byte $02 clears timer when written to IE
    cpu.sp = 0x0000;
    cpu.ime = true;

    let mut bus = Bus::from_rom(vec![0; 0x300]);
    bus.write8(IE_ADDR, Interrupt::Timer.mask());
    bus.write8(IF_ADDR, Interrupt::Timer.mask());

    assert!(poll(&mut cpu, &mut bus));
    assert_eq!(cpu.pc, 0x0000);
    assert!(!cpu.ime);
    assert_eq!(
        bus.read8(IF_ADDR) & Interrupt::Timer.mask(),
        Interrupt::Timer.mask()
    );
}

#[test]
fn ie_push_high_byte_can_redirect_to_lower_priority() {
    let mut cpu = Cpu::after_boot();
    cpu.pc = 0x0200; // $02 keeps STAT, clears VBlank
    cpu.sp = 0x0000;
    cpu.ime = true;

    let mut bus = Bus::from_rom(vec![0; 0x300]);
    bus.write8(
        IE_ADDR,
        Interrupt::VBlank.mask() | Interrupt::LcdStat.mask(),
    );
    bus.write8(
        IF_ADDR,
        Interrupt::VBlank.mask() | Interrupt::LcdStat.mask(),
    );

    assert!(poll(&mut cpu, &mut bus));
    assert_eq!(cpu.pc, Interrupt::LcdStat.vector());
    assert_eq!(
        bus.read8(IF_ADDR) & 0x1F,
        Interrupt::VBlank.mask(),
        "STAT cleared, VBlank remains"
    );
}

#[test]
fn ie_push_low_byte_is_too_late_to_cancel() {
    let mut cpu = Cpu::after_boot();
    // SP=$0001 → high push to $0000, low push to IE with PC low=$35 clearing serial.
    cpu.pc = 0x0235;
    cpu.sp = 0x0001;
    cpu.ime = true;

    let mut bus = Bus::from_rom(vec![0; 0x300]);
    bus.write8(IE_ADDR, Interrupt::Serial.mask());
    bus.write8(IF_ADDR, Interrupt::Serial.mask());

    assert!(poll(&mut cpu, &mut bus));
    assert_eq!(cpu.pc, Interrupt::Serial.vector());
    assert_eq!(bus.read8(IF_ADDR) & Interrupt::Serial.mask(), 0);
}
