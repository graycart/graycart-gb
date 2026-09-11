use super::*;
use crate::input::GameBoyButton;

const SEL_DPAD: u8 = 0x20; // bit4=0, bit5=1
const SEL_BUTTONS: u8 = 0x10; // bit4=1, bit5=0
const SEL_BOTH: u8 = 0x00;
const SEL_NONE: u8 = 0x30;

fn select_dpad(j: &mut Joypad) {
    j.write(SEL_DPAD);
}

fn select_buttons(j: &mut Joypad) {
    j.write(SEL_BUTTONS);
}

fn select_both(j: &mut Joypad) {
    j.write(SEL_BOTH);
}

fn select_none(j: &mut Joypad) {
    j.write(SEL_NONE);
}

#[test]
fn idle_reads_cf_both_groups_selected_released() {
    let j = Joypad::new();
    assert_eq!(j.read(), 0xCF);
}

#[test]
fn each_direction_maps_to_matrix_bit() {
    let cases = [
        (GameBoyButton::Right, 0),
        (GameBoyButton::Left, 1),
        (GameBoyButton::Up, 2),
        (GameBoyButton::Down, 3),
    ];
    for (button, bit) in cases {
        let mut j = Joypad::new();
        select_dpad(&mut j);
        assert!(j.press(button));
        assert_eq!(j.read() & 0x0F, !(1 << bit) & 0x0F);
        assert_eq!(j.read() & 0x30, SEL_DPAD);
    }
}

#[test]
fn each_action_maps_to_matrix_bit() {
    let cases = [
        (GameBoyButton::A, 0),
        (GameBoyButton::B, 1),
        (GameBoyButton::Select, 2),
        (GameBoyButton::Start, 3),
    ];
    for (button, bit) in cases {
        let mut j = Joypad::new();
        select_buttons(&mut j);
        assert!(j.press(button));
        assert_eq!(j.read() & 0x0F, !(1 << bit) & 0x0F);
        assert_eq!(j.read() & 0x30, SEL_BUTTONS);
    }
}

#[test]
fn direction_group_selection_hides_actions() {
    let mut j = Joypad::new();
    select_dpad(&mut j);
    j.press(GameBoyButton::A);
    assert_eq!(j.read() & 0x0F, 0x0F);
    j.press(GameBoyButton::Right);
    assert_eq!(j.read() & 0x0F, 0x0E);
}

#[test]
fn action_group_selection_hides_directions() {
    let mut j = Joypad::new();
    select_buttons(&mut j);
    j.press(GameBoyButton::Right);
    assert_eq!(j.read() & 0x0F, 0x0F);
    j.press(GameBoyButton::A);
    assert_eq!(j.read() & 0x0F, 0x0E);
}

#[test]
fn both_groups_selected_ands_lines() {
    let mut j = Joypad::new();
    select_both(&mut j);
    j.press(GameBoyButton::A);
    assert_eq!(j.read() & 0x0F, 0x0E);
    j.press(GameBoyButton::Right);
    assert_eq!(j.read() & 0x0F, 0x0E);
    j.release(GameBoyButton::A);
    assert_eq!(j.read() & 0x0F, 0x0E);
    j.release(GameBoyButton::Right);
    assert_eq!(j.read() & 0x0F, 0x0F);
}

#[test]
fn no_group_selected_returns_high_inputs() {
    let mut j = Joypad::new();
    j.press(GameBoyButton::A);
    j.press(GameBoyButton::Right);
    select_none(&mut j);
    assert_eq!(j.read() & 0x0F, 0x0F);
    assert_eq!(j.read(), 0xFF);
}

#[test]
fn press_release_round_trip() {
    let mut j = Joypad::new();
    select_buttons(&mut j);
    assert!(j.press(GameBoyButton::Start));
    assert!(j.is_pressed(GameBoyButton::Start));
    assert_eq!(j.read() & 0x0F, 0x07);
    assert!(!j.release(GameBoyButton::Start));
    assert!(!j.is_pressed(GameBoyButton::Start));
    assert_eq!(j.read() & 0x0F, 0x0F);
}

#[test]
fn irq_only_on_falling_edge_of_selected_line() {
    let mut j = Joypad::new();
    select_buttons(&mut j);
    assert!(j.press(GameBoyButton::A));
    assert!(!j.press(GameBoyButton::A));
    assert!(!j.release(GameBoyButton::A));
    assert!(!j.press(GameBoyButton::Right));
}

#[test]
fn unselected_press_then_select_can_fall() {
    let mut j = Joypad::new();
    select_none(&mut j);
    assert!(!j.press(GameBoyButton::A));
    assert!(j.write(SEL_BUTTONS));
    assert_eq!(j.read() & 0x0F, 0x0E);
}

#[test]
fn unused_bits_always_read_high() {
    let mut j = Joypad::new();
    j.write(0x00);
    assert_eq!(j.read() & 0xC0, 0xC0);
}

#[test]
fn both_selected_shared_line_blocks_second_edge() {
    let mut j = Joypad::new();
    select_both(&mut j);
    assert!(j.press(GameBoyButton::A));
    assert!(!j.press(GameBoyButton::Right));
}
