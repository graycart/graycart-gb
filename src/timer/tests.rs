use super::*;

#[test]
fn div_is_high_byte_and_resets_on_write() {
    let mut t = Timer::new();
    assert!(!t.tick(256)); // TIMA disabled
    assert_eq!(t.read(DIV), Some(0x01));
    t.write(DIV, 0xFF);
    assert_eq!(t.read(DIV), Some(0x00));
    assert_eq!(t.div, 0);
}

#[test]
fn tima_overflow_reads_zero_then_reloads_tma() {
    let mut t = Timer::new();
    t.write(TAC, TAC_ENABLE | 0b01); // 262144 Hz, bit 3
    t.write(TIMA, 0xFF);
    t.write(TMA, 0xAB);
    // From div=0, bit3 falls when 15→16.
    assert!(!t.tick(16));
    assert_eq!(t.tima, 0x00);
    assert_eq!(t.reload_delay, 4);
    assert!(!t.tick(3));
    assert_eq!(t.tima, 0x00);
    assert!(t.tick(1));
    assert_eq!(t.tima, 0xAB);
}

#[test]
fn tima_write_during_delay_cancels_reload_and_irq() {
    let mut t = Timer::new();
    t.write(TAC, TAC_ENABLE | 0b01);
    t.write(TIMA, 0xFF);
    t.write(TMA, 0xAB);
    assert!(!t.tick(16));
    assert_eq!(t.reload_delay, 4);
    t.write(TIMA, 0x55);
    assert_eq!(t.tima, 0x55);
    assert_eq!(t.reload_delay, 0);
    assert!(!t.tick(4));
    assert_eq!(t.tima, 0x55);
}

#[test]
fn tima_write_on_reload_cycle_is_ignored() {
    let mut t = Timer::new();
    t.write(TAC, TAC_ENABLE | 0b01);
    t.write(TIMA, 0xFF);
    t.write(TMA, 0xAB);
    assert!(!t.tick(16));
    assert!(!t.tick(3));
    assert_eq!(t.reload_delay, 1);
    t.write(TIMA, 0x55);
    assert_eq!(t.tima, 0x00); // write ignored
    assert!(t.tick(1));
    assert_eq!(t.tima, 0xAB);
}

#[test]
fn tma_write_on_reload_cycle_is_used() {
    let mut t = Timer::new();
    t.write(TAC, TAC_ENABLE | 0b01);
    t.write(TIMA, 0xFF);
    t.write(TMA, 0xAB);
    assert!(!t.tick(16));
    assert!(!t.tick(3));
    assert_eq!(t.reload_delay, 1);
    t.write(TMA, 0xCD);
    assert!(t.tick(1));
    assert_eq!(t.tima, 0xCD);
}

#[test]
fn freeze_div_stops_divider_and_tima() {
    let mut t = Timer::new();
    t.write(TAC, TAC_ENABLE | 0b01);
    t.write(TIMA, 0x10);
    t.write(DIV, 0);
    t.freeze_div(true);
    assert!(!t.tick(8200));
    assert_eq!(t.read(DIV), Some(0));
    assert_eq!(t.tima, 0x10);
    t.freeze_div(false);
    assert!(!t.tick(256));
    assert_eq!(t.read(DIV), Some(0x01));
}

#[test]
fn freeze_div_pauses_tima_reload_delay() {
    let mut t = Timer::new();
    t.write(TAC, TAC_ENABLE | 0b01);
    t.write(TIMA, 0xFF);
    t.write(TMA, 0xAB);
    assert!(!t.tick(16));
    assert_eq!(t.reload_delay, 4);
    t.freeze_div(true);
    assert!(!t.tick(4));
    assert_eq!(t.reload_delay, 4);
    assert_eq!(t.tima, 0);
    t.freeze_div(false);
    assert!(!t.tick(3));
    assert!(t.tick(1));
    assert_eq!(t.tima, 0xAB);
}
