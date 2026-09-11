use super::{BLOCK_LEN, block_cpu_t, block_fixed_t, hblank_may_transfer};
use crate::hw::ClockState;

#[test]
fn block_is_sixteen_bytes() {
    assert_eq!(BLOCK_LEN, 16);
}

#[test]
fn block_cpu_t_is_32_in_normal_speed() {
    assert_eq!(block_cpu_t(ClockState::Normal), 32);
}

#[test]
fn block_cpu_t_is_64_in_double_speed() {
    assert_eq!(block_cpu_t(ClockState::Double), 64);
}

#[test]
fn block_fixed_t_is_always_32() {
    assert_eq!(block_fixed_t(), 32);
}

#[test]
fn hblank_may_transfer_on_visible_ly_when_cpu_runs() {
    assert!(hblank_may_transfer(0, false));
    assert!(hblank_may_transfer(143, false));
}

#[test]
fn hblank_may_transfer_false_in_vblank() {
    assert!(!hblank_may_transfer(144, false));
    assert!(!hblank_may_transfer(153, false));
}

#[test]
fn hblank_may_transfer_false_while_halted() {
    assert!(!hblank_may_transfer(0, true));
    assert!(!hblank_may_transfer(143, true));
}
