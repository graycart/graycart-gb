use super::{ClockState, STOP_PAUSE_CPU_T};

#[test]
fn defaults_to_normal_speed() {
    assert_eq!(ClockState::default(), ClockState::Normal);
    assert!(!ClockState::Normal.is_double());
    assert!(ClockState::Double.is_double());
}

#[test]
fn toggle_swaps_normal_and_double() {
    assert_eq!(ClockState::Normal.toggle(), ClockState::Double);
    assert_eq!(ClockState::Double.toggle(), ClockState::Normal);
}

#[test]
fn stop_pause_is_2050_m_cycles_in_cpu_t() {
    assert_eq!(STOP_PAUSE_CPU_T, 8200);
}

#[test]
fn normal_maps_cpu_t_one_to_one_and_leaves_leftover_zero() {
    let mut leftover = 0u8;
    assert_eq!(ClockState::Normal.cpu_t_to_fixed_t(1, &mut leftover), 1);
    assert_eq!(leftover, 0);
    assert_eq!(ClockState::Normal.cpu_t_to_fixed_t(7, &mut leftover), 7);
    assert_eq!(leftover, 0);
}

#[test]
fn double_odd_cpu_t_carries_then_emits_one_fixed_t() {
    let mut leftover = 0u8;
    assert_eq!(ClockState::Double.cpu_t_to_fixed_t(1, &mut leftover), 0);
    assert_eq!(leftover, 1);
    assert_eq!(ClockState::Double.cpu_t_to_fixed_t(1, &mut leftover), 1);
    assert_eq!(leftover, 0);
}

#[test]
fn double_even_cpu_t_is_half_with_no_leftover() {
    let mut leftover = 0u8;
    assert_eq!(ClockState::Double.cpu_t_to_fixed_t(4, &mut leftover), 2);
    assert_eq!(leftover, 0);
}

#[test]
fn double_combines_leftover_with_cpu_t() {
    let mut leftover = 1u8;
    assert_eq!(ClockState::Double.cpu_t_to_fixed_t(3, &mut leftover), 2);
    assert_eq!(leftover, 0);
    leftover = 1;
    assert_eq!(ClockState::Double.cpu_t_to_fixed_t(2, &mut leftover), 1);
    assert_eq!(leftover, 1);
}
