//! Stack push/pop helpers shared by control flow and interrupt service.

use super::Cpu;
use crate::bus::Bus;

pub(crate) fn push16(cpu: &mut Cpu, bus: &mut Bus, value: u16) {
    let [lo, hi] = value.to_le_bytes();
    cpu.sp = cpu.sp.wrapping_sub(1);
    bus.write8(cpu.sp, hi);
    cpu.sp = cpu.sp.wrapping_sub(1);
    bus.write8(cpu.sp, lo);
}

pub(crate) fn pop16(cpu: &mut Cpu, bus: &Bus) -> u16 {
    let lo = bus.read8(cpu.sp);
    cpu.sp = cpu.sp.wrapping_add(1);
    let hi = bus.read8(cpu.sp);
    cpu.sp = cpu.sp.wrapping_add(1);
    u16::from_le_bytes([lo, hi])
}
