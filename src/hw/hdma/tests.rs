//! Wave 1 D acceptance matrix (types only; no Bus).
//!
//! Consumes `hdma.rs` re-exports so those `pub use`s are live under clippy.
//! [Pan Docs](https://gbdev.io/pandocs/CGB_Registers.html#lcd-vram-dma-transfers)

use super::{
    BLOCK_LEN, Block, HDMA1, HDMA2, HDMA3, HDMA4, HDMA5, Hdma5Write, StartOutcome, VramDma,
    VramDmaMode, block_cpu_t, block_fixed_t, hblank_may_transfer, mask_dest, mask_source,
    parse_hdma5_write, read_hdma5, source_is_vram,
};
use crate::hw::ClockState;

fn latch_rom_to_vram(dma: &mut VramDma) {
    dma.write_src_high(0x12);
    dma.write_src_low(0x34);
    dma.write_dest_high(0x80);
    dma.write_dest_low(0x00);
}

/// D1 — HDMA1–5 addresses are `$FF51`–`$FF55`.
#[test]
fn d1_hdma_register_addresses() {
    let ports = [
        (HDMA1, 0xFF51),
        (HDMA2, 0xFF52),
        (HDMA3, 0xFF53),
        (HDMA4, 0xFF54),
        (HDMA5, 0xFF55),
    ];
    for (got, expect) in ports {
        assert_eq!(got, expect);
    }
}

/// D2 — source low nibble forced 0 (`mask_source`).
#[test]
fn d2_source_mask_clears_low_nibble() {
    let cases = [
        (0x0000, 0x0000),
        (0x000F, 0x0000),
        (0x1234, 0x1230),
        (0x7FFF, 0x7FF0),
        (0xA00F, 0xA000),
        (0xDFF0, 0xDFF0),
        (0xFFFF, 0xFFF0),
    ];
    for (raw, masked) in cases {
        assert_eq!(mask_source(raw), masked, "mask_source({raw:#06X})");
        assert_eq!(mask_source(raw) & 0x000F, 0);
    }
}

/// D3 — dest always VRAM `$8000`–`$9FF0` (bits 15–13 and 3–0 ignored).
#[test]
fn d3_dest_mask_is_vram_8000_9ff0() {
    let cases = [
        (0x0000, 0x8000),
        (0x0005, 0x8000),
        (0x1FF0, 0x9FF0),
        (0x8000, 0x8000),
        (0x9FF0, 0x9FF0),
        (0xFFFF, 0x9FF0),
        (0xE005, 0x8000),
    ];
    for (raw, masked) in cases {
        let dest = mask_dest(raw);
        assert_eq!(dest, masked, "mask_dest({raw:#06X})");
        assert!((0x8000..=0x9FF0).contains(&dest));
        assert_eq!(dest & 0x000F, 0);
    }
}

/// D4 — VRAM source (`$8000`–`$9FFF` after mask) is garbage.
#[test]
fn d4_vram_source_is_garbage() {
    let vram_src = [0x8000, 0x800F, 0x9FF0, 0x9FFF];
    for addr in vram_src {
        assert!(source_is_vram(addr), "{addr:#06X}");
    }
    for addr in [0x7FF0, 0xA000, 0x0000, 0xC000] {
        assert!(!source_is_vram(addr), "{addr:#06X}");
    }

    let mut dma = VramDma::new();
    dma.write_src_high(0x80);
    dma.write_src_low(0x0F);
    dma.write_dest_high(0x90);
    dma.write_dest_low(0x00);
    dma.start_from_hdma5(0x00, false);
    let block = dma.take_block().expect("GDMA block");
    assert_eq!(
        block,
        Block {
            src: 0x8000,
            dest: 0x9000,
            garbage_src: true,
        }
    );
}

/// D5 — GDMA length `$00` is one `$10` block, then idle and HDMA5=`$FF`.
#[test]
fn d5_gdma_one_block_then_idle() {
    assert_eq!(BLOCK_LEN, 16);
    let parsed = parse_hdma5_write(0x00);
    assert_eq!(
        parsed,
        Hdma5Write {
            blocks_minus_1: 0,
            hblank: false,
        }
    );

    let mut dma = VramDma::new();
    latch_rom_to_vram(&mut dma);
    assert_eq!(dma.start_from_hdma5(0x00, false), StartOutcome::StartedGdma);
    assert_eq!(dma.mode(), VramDmaMode::Gdma);
    assert_eq!(dma.remaining_blocks(), 1);

    let block = dma.take_block().expect("one GDMA block");
    assert_eq!(block.src, 0x1230);
    assert_eq!(block.dest, 0x8000);
    assert!(!block.garbage_src);

    assert_eq!(dma.mode(), VramDmaMode::Idle);
    assert_eq!(dma.remaining_blocks(), 0);
    assert!(dma.take_block().is_none());
    assert_eq!(dma.hdma5_read(), 0xFF);
    assert_eq!(read_hdma5(0xFF, false), 0xFF);
}

/// D6 — GDMA `$7F` is `$80` blocks then idle (loop `take_block`; no memcpy).
#[test]
fn d6_gdma_0x7f_is_0x80_blocks_then_idle() {
    let parsed = parse_hdma5_write(0x7F);
    assert_eq!(parsed.blocks_minus_1, 0x7F);
    assert!(!parsed.hblank);

    let mut dma = VramDma::new();
    latch_rom_to_vram(&mut dma);
    assert_eq!(dma.start_from_hdma5(0x7F, false), StartOutcome::StartedGdma);
    assert_eq!(dma.remaining_blocks(), 0x80);

    let mut n = 0u16;
    while dma.take_block().is_some() {
        n += 1;
    }
    assert_eq!(n, 0x80);
    assert_eq!(dma.mode(), VramDmaMode::Idle);
    assert_eq!(dma.remaining_blocks(), 0);
    assert_eq!(dma.hdma5_read(), 0xFF);
}

/// D7 — HDMA waits: `take_block` is None until `allow_next_hblank_block`.
#[test]
fn d7_hdma_waits_until_allow() {
    let mut dma = VramDma::new();
    latch_rom_to_vram(&mut dma);
    assert_eq!(
        dma.start_from_hdma5(0x81, false),
        StartOutcome::StartedHdma {
            defer_this_hblank: false
        }
    );
    assert_eq!(dma.mode(), VramDmaMode::Hdma);
    assert!(dma.take_block().is_none());

    dma.allow_next_hblank_block();
    assert!(dma.take_block().is_some());
}

/// D8 — HDMA start in HBlank: no copy at FF55; first Mode 0 enter copies one block.
#[test]
fn d8_hdma_start_in_hblank_first_allow_copies() {
    let mut dma = VramDma::new();
    latch_rom_to_vram(&mut dma);
    assert_eq!(
        dma.start_from_hdma5(0x80, true),
        StartOutcome::StartedHdma {
            defer_this_hblank: true
        }
    );
    assert!(
        dma.take_block().is_none(),
        "mid-HBlank start must not copy at FF55"
    );

    dma.allow_next_hblank_block();
    let block = dma.take_block().expect("next Mode 0 enter copies");
    assert_eq!(block.src, 0x1230);
    assert_eq!(block.dest, 0x8000);
    assert_eq!(dma.mode(), VramDmaMode::Idle);
}

/// D9 — VBlank LY: `hblank_may_transfer(144/153)` is false (timing fn only; no LY on the model).
#[test]
fn d9_vblank_ly_does_not_allow_hdma() {
    for ly in [144u8, 153] {
        assert!(!hblank_may_transfer(ly, false), "LY={ly}");
    }
    assert!(hblank_may_transfer(0, false));
    assert!(hblank_may_transfer(143, false));
}

/// D10 — HALT pauses HDMA (`hblank_may_transfer(0, true)` is false).
#[test]
fn d10_halt_pauses_hdma() {
    assert!(!hblank_may_transfer(0, true));
    assert!(!hblank_may_transfer(80, true));
}

/// D11 — abort HDMA via HDMA5 bit7=0: read bit7=1, source/dest latches unchanged.
#[test]
fn d11_abort_sets_bit7_and_keeps_latches() {
    let mut dma = VramDma::new();
    dma.write_src_high(0x40);
    dma.write_src_low(0x10);
    dma.write_dest_high(0x90);
    dma.write_dest_low(0x20);

    dma.start_from_hdma5(0x83, false);
    let outcome = dma.start_from_hdma5(0x00, false);
    assert_eq!(
        outcome,
        StartOutcome::AbortedHdma {
            remaining_minus_1: 3
        }
    );
    assert_eq!(dma.mode(), VramDmaMode::Idle);
    let hdma5 = dma.hdma5_read();
    assert_eq!(hdma5 & 0x80, 0x80, "abort reads bit7=1");
    assert_eq!(hdma5, 0x83);
    assert_eq!(read_hdma5(3, false), 0x83);

    dma.start_from_hdma5(0x80, false);
    dma.allow_next_hblank_block();
    let block = dma.take_block().expect("latches survive abort");
    assert_eq!(block.src, 0x4010);
    assert_eq!(block.dest, 0x9020);
}

/// D12 — one `$10` burst: CPU T 32/64; fixed (PPU) T always 32.
#[test]
fn d12_cpu_t_vs_fixed_t() {
    let cases = [(ClockState::Normal, 32, 32), (ClockState::Double, 64, 32)];
    for (clock, cpu_t, fixed_t) in cases {
        assert_eq!(block_cpu_t(clock), cpu_t, "{clock:?} CPU T");
        assert_eq!(block_fixed_t(), fixed_t, "{clock:?} fixed T");
    }
}

/// D13 — HDMA is not dump-all: after one allow+take, remaining > 0 if length > 1.
#[test]
fn d13_hdma_is_not_dump_all() {
    let mut dma = VramDma::new();
    latch_rom_to_vram(&mut dma);
    dma.start_from_hdma5(0x81, false);
    assert_eq!(dma.remaining_blocks(), 2);

    dma.allow_next_hblank_block();
    assert!(dma.take_block().is_some());
    assert!(dma.remaining_blocks() > 0);
    assert_eq!(dma.remaining_blocks(), 1);
    assert_eq!(dma.mode(), VramDmaMode::Hdma);
    assert!(
        dma.take_block().is_none(),
        "second block needs another HBlank"
    );
}
