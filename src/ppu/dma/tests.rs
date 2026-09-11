use super::*;

#[test]
fn fresh_dma_waits_one_m_cycle_before_first_byte() {
    let mut dma = OamDma::new();
    dma.start(0x80);
    assert!(!dma.blocks_oam());
    assert_eq!(dma.tick(4), Vec::<u16>::new());
    assert!(dma.blocks_oam());
    assert_eq!(dma.tick(4), vec![0x8000]);
}

#[test]
fn dma_copies_one_byte_per_m_cycle_after_delay() {
    let mut dma = OamDma::new();
    dma.start(0xC0);
    assert!(dma.tick(4).is_empty());
    assert_eq!(dma.tick(4), vec![0xC000]);
    assert_eq!(dma.tick(4), vec![0xC001]);
    assert_eq!(dma.tick(8), vec![0xC002, 0xC003]);
}

#[test]
fn dma_completes_after_delay_plus_160_bytes() {
    let mut dma = OamDma::new();
    dma.start(0x80);
    let addrs = dma.tick(START_DELAY_T + u32::from(DMA_BYTE_COUNT) * 4);
    assert_eq!(addrs.len(), usize::from(DMA_BYTE_COUNT));
    assert_eq!(addrs[0], 0x8000);
    assert_eq!(addrs[159], 0x809F);
    assert!(!dma.active());
    assert!(!dma.blocks_oam());
}

#[test]
fn restart_continues_old_dma_one_m_cycle() {
    let mut dma = OamDma::new();
    dma.start(0x80);
    assert!(dma.tick(4).is_empty()); // delay
    assert_eq!(dma.tick(4), vec![0x8000]); // byte 0 of first DMA

    // Restart mid-transfer.
    dma.start(0xC0);
    assert!(dma.blocks_oam());
    // Old DMA still copies one more byte during the restart delay.
    assert_eq!(dma.tick(4), vec![0x8001]);
    // New DMA begins.
    assert_eq!(dma.tick(4), vec![0xC000]);
    assert_eq!(dma.tick(4), vec![0xC001]);
}

#[test]
fn fresh_start_after_completion_resets_cleanly() {
    let mut dma = OamDma::new();
    dma.start(0x80);
    let _ = dma.tick(START_DELAY_T + u32::from(DMA_BYTE_COUNT) * 4);
    assert!(!dma.active());
    dma.start(0x90);
    assert!(!dma.blocks_oam());
    assert!(dma.tick(4).is_empty());
    assert!(dma.blocks_oam());
    assert_eq!(dma.tick(4), vec![0x9000]);
}
