//! Exhaustive CGB BG-vs-OBJ priority table.
//!
//! Pan Docs Tile Maps + LCDC bit 0 (CGB master priority):
//! 1. BG color index 0 ⇒ OBJ wins
//! 2. Else if LCDC.0 clear ⇒ OBJ wins
//! 3. Else if both BG attr.7 and OAM attr.7 are clear ⇒ OBJ wins
//! 4. Else BG (indices 1–3) wins

use super::cgb_obj_wins_over_bg;

/// Pan Docs 8-row flag table, plus “BG color 1–3, otherwise OBJ”.
fn expected_obj_wins(
    bg_color_id: u8,
    bg_attr_priority: bool,
    obj_bg_over: bool,
    lcdc0: bool,
) -> bool {
    if bg_color_id == 0 {
        return true;
    }
    match (lcdc0, obj_bg_over, bg_attr_priority) {
        (false, false, false) => true,
        (false, false, true) => true,
        (false, true, false) => true,
        (false, true, true) => true,
        (true, false, false) => true,
        (true, false, true) => false,
        (true, true, false) => false,
        (true, true, true) => false,
    }
}

#[test]
fn rule1_bg_color_0_obj_always_wins() {
    for lcdc0 in [false, true] {
        for bg_attr_priority in [false, true] {
            for obj_bg_over in [false, true] {
                assert!(
                    cgb_obj_wins_over_bg(0, bg_attr_priority, obj_bg_over, lcdc0),
                    "color 0 lcdc0={lcdc0} bg.7={bg_attr_priority} oam.7={obj_bg_over}"
                );
            }
        }
    }
}

#[test]
fn rule2_lcdc0_clear_obj_wins_for_bg_indices_1_3() {
    for color in 1..=3 {
        for bg_attr_priority in [false, true] {
            for obj_bg_over in [false, true] {
                assert!(
                    cgb_obj_wins_over_bg(color, bg_attr_priority, obj_bg_over, false),
                    "color {color} lcdc0=0 bg.7={bg_attr_priority} oam.7={obj_bg_over}"
                );
            }
        }
    }
}

#[test]
fn rule3_lcdc0_set_both_priority_bits_clear_obj_wins() {
    for color in 1..=3 {
        assert!(
            cgb_obj_wins_over_bg(color, false, false, true),
            "color {color} both attr.7 clear"
        );
    }
}

#[test]
fn rule4_lcdc0_set_either_priority_bit_bg_wins_for_indices_1_3() {
    for color in 1..=3 {
        for (bg_attr_priority, obj_bg_over) in [(true, false), (false, true), (true, true)] {
            assert!(
                !cgb_obj_wins_over_bg(color, bg_attr_priority, obj_bg_over, true),
                "color {color} lcdc0=1 bg.7={bg_attr_priority} oam.7={obj_bg_over}"
            );
        }
    }
}

#[test]
fn pan_docs_truth_table_all_flag_and_color_combinations() {
    for lcdc0 in [false, true] {
        for obj_bg_over in [false, true] {
            for bg_attr_priority in [false, true] {
                for bg_color_id in 0..=3 {
                    let obj_wins =
                        cgb_obj_wins_over_bg(bg_color_id, bg_attr_priority, obj_bg_over, lcdc0);
                    let expect =
                        expected_obj_wins(bg_color_id, bg_attr_priority, obj_bg_over, lcdc0);
                    assert_eq!(
                        obj_wins, expect,
                        "lcdc0={lcdc0} oam.7={obj_bg_over} bg.7={bg_attr_priority} color={bg_color_id}"
                    );
                }
            }
        }
    }
}
