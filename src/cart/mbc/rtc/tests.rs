use super::*;

#[test]
fn advance_seconds_rolls_minutes_hours_days() {
    let mut rtc = Rtc::new();
    rtc.write_reg(0x08, 58);
    rtc.write_reg(0x09, 59);
    rtc.write_reg(0x0A, 23);
    rtc.write_reg(0x0B, 0x00);
    rtc.write_reg(0x0C, 0x00); // running

    rtc.advance_seconds(5); // 23:59:58 + 5s → day1 00:00:03
    rtc.write_latch(0);
    rtc.write_latch(1);
    assert_eq!(rtc.read_latched(0x08), 3);
    assert_eq!(rtc.read_latched(0x09), 0);
    assert_eq!(rtc.read_latched(0x0A), 0);
    assert_eq!(rtc.read_latched(0x0B), 1);
}

#[test]
fn halt_stops_advance() {
    let mut rtc = Rtc::new();
    rtc.write_reg(0x08, 10);
    rtc.write_reg(0x0C, 0x40); // halt
    rtc.advance_seconds(100);
    rtc.write_latch(0);
    rtc.write_latch(1);
    assert_eq!(rtc.read_latched(0x08), 10);
}

#[test]
fn latch_requires_zero_then_one() {
    let mut rtc = Rtc::new();
    rtc.write_reg(0x08, 42);
    rtc.write_latch(1); // no prior 0
    assert_eq!(rtc.read_latched(0x08), 0);
    rtc.write_latch(0);
    rtc.write_latch(1);
    assert_eq!(rtc.read_latched(0x08), 42);
}

#[test]
fn day_overflow_sets_carry() {
    let mut rtc = Rtc::new();
    rtc.write_reg(0x08, 0);
    rtc.write_reg(0x09, 0);
    rtc.write_reg(0x0A, 0);
    rtc.write_reg(0x0B, 0xFF);
    rtc.write_reg(0x0C, 0x01); // day = 511
    rtc.advance_seconds(24 * 60 * 60); // +1 day
    rtc.write_latch(0);
    rtc.write_latch(1);
    assert_eq!(rtc.read_latched(0x0B), 0);
    assert_eq!(rtc.read_latched(0x0C) & 0x01, 0);
    assert_ne!(rtc.read_latched(0x0C) & 0x80, 0);
}

#[test]
fn tick_accumulates_cpu_cycles() {
    let mut rtc = Rtc::new();
    rtc.tick(T_CYCLES_PER_SECOND as u32 - 1);
    rtc.write_latch(0);
    rtc.write_latch(1);
    assert_eq!(rtc.read_latched(0x08), 0);
    rtc.tick(1);
    rtc.write_latch(0);
    rtc.write_latch(1);
    assert_eq!(rtc.read_latched(0x08), 1);
}

#[test]
fn save_round_trip() {
    let mut rtc = Rtc::new();
    rtc.write_reg(0x08, 11);
    rtc.write_reg(0x09, 22);
    rtc.write_reg(0x0A, 3);
    rtc.write_reg(0x0B, 4);
    rtc.write_reg(0x0C, 0x41);
    rtc.write_latch(0);
    rtc.write_latch(1);
    rtc.set_unix_secs(1_700_000_000);
    let bytes = rtc.to_save_bytes();

    let mut loaded = Rtc::new();
    loaded.load_save_bytes(&bytes);
    assert_eq!(loaded.read_latched(0x08), 11);
    assert_eq!(loaded.unix_secs(), 1_700_000_000);
    // Running regs restored
    loaded.write_latch(0);
    loaded.write_latch(1);
    assert_eq!(loaded.read_latched(0x09), 22);
}
