//! VRAM DMA transfer model (no memcpy, no Bus).
//!
//! `allow_next_hblank_block` is Mode 0 **enter** only (Wave 2 G). Starting HDMA
//! mid-HBlank must not copy at `$FF55`; the **next** Mode 0 enter copies.

use super::{Block, StartOutcome, VramDma, VramDmaMode};

fn latch_rom_to_vram(dma: &mut VramDma) {
    dma.write_src_high(0x12);
    dma.write_src_low(0x34);
    dma.write_dest_high(0x80);
    dma.write_dest_low(0x00);
}

#[test]
fn writes_do_not_start_a_transfer() {
    let mut dma = VramDma::new();
    latch_rom_to_vram(&mut dma);
    assert_eq!(dma.mode(), VramDmaMode::Idle);
    assert_eq!(dma.remaining_blocks(), 0);
    assert!(dma.take_block().is_none());
    assert_eq!(dma.hdma5_read(), 0xFF);
}

#[test]
fn gdma_length_zero_is_one_block_then_idle() {
    let mut dma = VramDma::new();
    latch_rom_to_vram(&mut dma);
    assert_eq!(dma.start_from_hdma5(0x00, false), StartOutcome::StartedGdma);
    assert_eq!(dma.mode(), VramDmaMode::Gdma);
    assert_eq!(dma.remaining_blocks(), 1);

    let block = dma.take_block().expect("GDMA block available immediately");
    assert_eq!(
        block,
        Block {
            src: 0x1230,
            dest: 0x8000,
            garbage_src: false,
        }
    );
    assert_eq!(dma.mode(), VramDmaMode::Idle);
    assert_eq!(dma.remaining_blocks(), 0);
    assert!(dma.take_block().is_none());
    assert_eq!(dma.hdma5_read(), 0xFF);
}

#[test]
fn hdma_not_in_hblank_waits_for_allow() {
    let mut dma = VramDma::new();
    latch_rom_to_vram(&mut dma);
    assert_eq!(
        dma.start_from_hdma5(0x81, false),
        StartOutcome::StartedHdma {
            defer_this_hblank: false
        }
    );
    assert_eq!(dma.mode(), VramDmaMode::Hdma);
    assert_eq!(dma.remaining_blocks(), 2);
    assert!(
        dma.take_block().is_none(),
        "HDMA does not copy at FF55 write"
    );

    dma.allow_next_hblank_block();
    let first = dma.take_block().expect("first HBlank after start");
    assert_eq!(first.src, 0x1230);
    assert_eq!(first.dest, 0x8000);
    assert_eq!(dma.mode(), VramDmaMode::Hdma);
    assert_eq!(dma.remaining_blocks(), 1);
    assert!(
        dma.take_block().is_none(),
        "HDMA is not GDMA: only one $10 block per HBlank"
    );

    dma.allow_next_hblank_block();
    assert!(dma.take_block().is_some());
    assert_eq!(dma.mode(), VramDmaMode::Idle);
}

#[test]
fn hdma_started_in_hblank_copies_on_first_mode0_enter() {
    // currently_hblank: do not copy at FF55 (this HBlank is already open).
    // Wave 2 G only calls allow_next_hblank_block on Mode 0 *enter*, so the
    // first allow is the next visible line and must arm a block.
    let mut dma = VramDma::new();
    latch_rom_to_vram(&mut dma);
    assert_eq!(
        dma.start_from_hdma5(0x80, true),
        StartOutcome::StartedHdma {
            defer_this_hblank: true
        }
    );
    assert_eq!(dma.mode(), VramDmaMode::Hdma);
    assert!(
        dma.take_block().is_none(),
        "mid-HBlank start must not copy at FF55"
    );

    dma.allow_next_hblank_block();
    let block = dma.take_block().expect("next Mode 0 enter copies");
    assert_eq!(block.src, 0x1230);
    assert_eq!(dma.mode(), VramDmaMode::Idle);
}

#[test]
fn abort_hdma_reports_remaining_minus_1_and_keeps_latches() {
    let mut dma = VramDma::new();
    dma.write_src_high(0x40);
    dma.write_src_low(0x10);
    dma.write_dest_high(0x90);
    dma.write_dest_low(0x20);

    assert_eq!(
        dma.start_from_hdma5(0x83, false),
        StartOutcome::StartedHdma {
            defer_this_hblank: false
        }
    );
    assert_eq!(dma.remaining_blocks(), 4);

    let outcome = dma.start_from_hdma5(0x00, false);
    assert_eq!(
        outcome,
        StartOutcome::AbortedHdma {
            remaining_minus_1: 3
        }
    );
    assert_eq!(dma.mode(), VramDmaMode::Idle);
    assert_eq!(dma.remaining_blocks(), 0);
    assert_eq!(dma.hdma5_read(), 0x83);

    dma.start_from_hdma5(0x80, false);
    dma.allow_next_hblank_block();
    let block = dma.take_block().expect("latches survive abort");
    assert_eq!(block.src, 0x4010);
    assert_eq!(block.dest, 0x9020);
}

#[test]
fn abort_hdma_method_encodes_remaining() {
    let mut dma = VramDma::new();
    latch_rom_to_vram(&mut dma);
    dma.start_from_hdma5(0x81, false);
    assert_eq!(dma.abort_hdma(), 1);
    assert_eq!(dma.mode(), VramDmaMode::Idle);
    assert_eq!(dma.remaining_blocks(), 0);
    assert_eq!(dma.hdma5_read(), 0x81);
}

#[test]
fn vram_source_sets_garbage_src() {
    let mut dma = VramDma::new();
    dma.write_src_high(0x80);
    dma.write_src_low(0x00);
    dma.write_dest_high(0x90);
    dma.write_dest_low(0x00);
    dma.start_from_hdma5(0x00, false);
    let block = dma.take_block().expect("GDMA");
    assert!(block.garbage_src);
    assert_eq!(block.src, 0x8000);
}

#[test]
fn dest_is_always_vram_after_mask() {
    let mut dma = VramDma::new();
    dma.write_src_high(0x00);
    dma.write_src_low(0x00);
    dma.write_dest_high(0x00);
    dma.write_dest_low(0x05);
    dma.start_from_hdma5(0x00, false);
    let low = dma.take_block().unwrap();
    assert_eq!(low.dest, 0x8000);
    assert!((0x8000..=0x9FF0).contains(&low.dest));
    assert_eq!(low.dest & 0x000F, 0);

    let mut dma = VramDma::new();
    dma.write_src_high(0x00);
    dma.write_src_low(0x00);
    dma.write_dest_high(0xFF);
    dma.write_dest_low(0xFF);
    dma.start_from_hdma5(0x00, false);
    let high = dma.take_block().unwrap();
    assert_eq!(high.dest, 0x9FF0);
    assert!((0x8000..=0x9FF0).contains(&high.dest));
    assert_eq!(high.dest & 0x000F, 0);
}

#[test]
fn gdma_can_take_successive_blocks_without_hblank() {
    let mut dma = VramDma::new();
    latch_rom_to_vram(&mut dma);
    dma.start_from_hdma5(0x01, false);
    let a = dma.take_block().unwrap();
    let b = dma.take_block().unwrap();
    assert_eq!(a.src, 0x1230);
    assert_eq!(b.src, 0x1240);
    assert_eq!(b.dest, 0x8010);
    assert_eq!(dma.mode(), VramDmaMode::Idle);
}
