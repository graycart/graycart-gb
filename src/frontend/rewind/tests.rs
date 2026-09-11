use super::*;
use graycart::{Bus, Cpu, MachineStateV1, capture};

fn dummy_state(heap: usize) -> MachineStateV1 {
    let mut state = capture(&Cpu::new(), &Bus::from_rom(vec![0u8; 0x8000]));
    state.cart.ram = vec![0; heap];
    state
}

#[test]
fn rewind_ring_evicts_oldest_when_over_soft_cap() {
    let template = dummy_state(64 * 1024);
    let mut ring = RewindRing::new(&template);
    let per = template.heap_bytes();
    let n = (REWIND_SOFT_CAP_BYTES / per).max(2) + 2;
    for i in 0..n {
        let mut s = template.clone();
        s.cpu.pc = i as u16;
        ring.push(s);
    }
    assert!(ring.len() <= n);
    assert_ne!(ring.oldest_pc(), 0);
}

#[test]
fn scrub_back_returns_previous_states_in_order() {
    let template = dummy_state(1024);
    let mut ring = RewindRing::new(&template);
    for i in 1..=3u16 {
        let mut s = template.clone();
        s.cpu.pc = i;
        ring.push(s);
    }
    ring.begin_scrub();
    assert_eq!(ring.scrub_back().unwrap().cpu.pc, 2);
    assert_eq!(ring.scrub_back().unwrap().cpu.pc, 1);
}

#[test]
fn end_scrub_truncates_ring_to_scrubbed_position() {
    let template = dummy_state(1024);
    let mut ring = RewindRing::new(&template);
    for i in 1..=3u16 {
        let mut s = template.clone();
        s.cpu.pc = i;
        ring.push(s);
    }
    ring.begin_scrub();
    assert_eq!(ring.scrub_back().unwrap().cpu.pc, 2);
    assert_eq!(ring.scrub_back().unwrap().cpu.pc, 1);
    ring.end_scrub();
    assert_eq!(ring.len(), 1);
    assert_eq!(ring.newest_pc(), 1);
    ring.begin_scrub();
    assert_eq!(ring.newest_pc(), 1);
    assert!(ring.scrub_back().is_none());
}

#[test]
fn enabled_follows_settings_flag() {
    let mut settings = FrontendSettings::default();
    assert!(!enabled(&settings));
    settings.rewind_enabled = true;
    assert!(enabled(&settings));
}
