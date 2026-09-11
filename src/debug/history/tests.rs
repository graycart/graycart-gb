use super::*;

#[test]
fn history_drops_oldest_past_capacity() {
    let mut h = InstructionHistory::new();
    for i in 0..(HISTORY_LEN + 3) {
        h.push(HistoryEntry {
            pc: i as u16,
            rom_bank: 0,
            bytes: [0, 0, 0],
            mnemonic: "NOP".into(),
            af: 0,
            bc: 0,
            de: 0,
            hl: 0,
            sp: 0,
            ime: false,
        });
    }
    assert_eq!(h.len(), HISTORY_LEN);
    assert_eq!(h.iter().next().unwrap().pc, 3);
    assert_eq!(h.iter().last().unwrap().pc, (HISTORY_LEN + 2) as u16);
}
