//! Mooneye Test Suite harness ([Gekkio/mooneye-test-suite](https://github.com/Gekkio/mooneye-test-suite)).
//!
//! Selective acceptance ROMs under `tests/fixtures/mooneye/acceptance/`.

use crate::harness::{Outcome, run_mooneye};
use std::path::{Path, PathBuf};

const FIXTURE_ROOT: &str = "tests/fixtures/mooneye";
const STEP_LIMIT: u32 = 5_000_000;

/// Start with timer / interrupt / OAM DMA / bits / PPU access — expand as accuracy grows.
const SUITE: &[&str] = &[
    "acceptance/bits/mem_oam.gb",
    "acceptance/bits/reg_f.gb",
    "acceptance/bits/unused_hwio-GS.gb",
    "acceptance/timer/tim00.gb",
    "acceptance/timer/tim01.gb",
    "acceptance/timer/tim10.gb",
    "acceptance/timer/tim11.gb",
    "acceptance/timer/tima_reload.gb",
    "acceptance/timer/div_write.gb",
    "acceptance/interrupts/ie_push.gb",
    "acceptance/oam_dma/basic.gb",
    "acceptance/oam_dma/reg_read.gb",
    "acceptance/oam_dma/sources-GS.gb",
    // 7D — OAM DMA start delay / restart / timing
    "acceptance/oam_dma_start.gb",
    "acceptance/oam_dma_timing.gb",
    "acceptance/oam_dma_restart.gb",
    "acceptance/halt_ime0_ei.gb",
    "acceptance/halt_ime1_timing.gb",
    "acceptance/ei_timing.gb",
    "acceptance/if_ie_registers.gb",
    "acceptance/intr_timing.gb",
    "acceptance/div_timing.gb",
    // 7B — OAM unlock after Mode 2 STAT (CPU access restrictions; needs 7C IRQ latency)
    "acceptance/ppu/intr_2_oam_ok_timing.gb",
    // 7C — Mode 3 length / STAT edges
    "acceptance/ppu/intr_2_mode3_timing.gb",
    "acceptance/ppu/hblank_ly_scx_timing-GS.gb",
];

fn fixture(rel: &str) -> PathBuf {
    Path::new(FIXTURE_ROOT).join(rel)
}

fn print_row(name: &str, outcome: &Outcome) {
    let detail = outcome.detail();
    if detail.is_empty() {
        eprintln!("{:<48} {}", name, outcome.label());
    } else {
        eprintln!("{:<48} {}  ({detail})", name, outcome.label());
    }
}

#[test]
fn mooneye_acceptance_fixtures_present() {
    let mut missing = Vec::new();
    for rel in SUITE {
        let p = fixture(rel);
        if !p.exists() {
            missing.push(p.display().to_string());
        }
    }
    assert!(
        missing.is_empty(),
        "missing Mooneye fixtures:\n{}",
        missing.join("\n")
    );
}

#[test]
#[ignore = "Mooneye acceptance matrix; run with --ignored --nocapture"]
fn mooneye_acceptance_matrix() {
    eprintln!();
    eprintln!("{:<48} result", "ROM");
    let mut counts = [0usize; 4]; // pass fail timeout unsupported
    for rel in SUITE {
        let path = fixture(rel);
        let outcome = run_mooneye(&path, STEP_LIMIT);
        print_row(rel, &outcome);
        match outcome {
            Outcome::Pass => counts[0] += 1,
            Outcome::Fail(_) => counts[1] += 1,
            Outcome::Timeout => counts[2] += 1,
            Outcome::Unsupported(_) => counts[3] += 1,
        }
    }
    eprintln!();
    eprintln!(
        "summary: PASS={} FAIL={} TIMEOUT={} UNSUPPORTED={}",
        counts[0], counts[1], counts[2], counts[3]
    );
    // Soft gate for now: fixtures must run; full green is Phase 7G.
    assert!(
        counts[3] == 0,
        "Mooneye suite hit UNSUPPORTED (missing CPU/hardware)"
    );
}
