//! Blargg test ROM harness ([retrio/gb-test-roms](https://github.com/retrio/gb-test-roms)).
//!
//! Fixtures live under `tests/fixtures/blargg/`. Targets: `cpu_instrs`, `dmg_sound`, `cgb_sound`.

use crate::harness::{Outcome, RomLaunchMode, run_blargg, run_blargg_launched};
use std::path::{Path, PathBuf};

const FIXTURE_ROOT: &str = "tests/fixtures/blargg";
/// Generous budget — individual cpu_instrs ROMs usually finish well under this.
const STEP_LIMIT: u32 = 50_000_000;
/// Sound tests are shorter but still need room for frame-sequencer waits.
const SOUND_STEP_LIMIT: u32 = 20_000_000;

const CPU_INSTRS_INDIVIDUAL: &[&str] = &[
    "cpu_instrs/individual/01-special.gb",
    "cpu_instrs/individual/02-interrupts.gb",
    "cpu_instrs/individual/03-op sp,hl.gb",
    "cpu_instrs/individual/04-op r,imm.gb",
    "cpu_instrs/individual/05-op rp.gb",
    "cpu_instrs/individual/06-ld r,r.gb",
    "cpu_instrs/individual/07-jr,jp,call,ret,rst.gb",
    "cpu_instrs/individual/08-misc instrs.gb",
    "cpu_instrs/individual/09-op r,r.gb",
    "cpu_instrs/individual/10-bit ops.gb",
    "cpu_instrs/individual/11-op a,(hl).gb",
];

const CGB_SOUND_SINGLES: &[&str] = &[
    "cgb_sound/rom_singles/01-registers.gb",
    "cgb_sound/rom_singles/02-len ctr.gb",
    "cgb_sound/rom_singles/03-trigger.gb",
    "cgb_sound/rom_singles/04-sweep.gb",
    "cgb_sound/rom_singles/05-sweep details.gb",
    "cgb_sound/rom_singles/06-overflow on trigger.gb",
    "cgb_sound/rom_singles/07-len sweep period sync.gb",
    "cgb_sound/rom_singles/08-len ctr during power.gb",
    "cgb_sound/rom_singles/09-wave read while on.gb",
    "cgb_sound/rom_singles/10-wave trigger while on.gb",
    "cgb_sound/rom_singles/11-regs after power.gb",
    "cgb_sound/rom_singles/12-wave.gb",
];

const DMG_SOUND_SINGLES: &[&str] = &[
    "dmg_sound/rom_singles/01-registers.gb",
    "dmg_sound/rom_singles/02-len ctr.gb",
    "dmg_sound/rom_singles/03-trigger.gb",
    "dmg_sound/rom_singles/04-sweep.gb",
    "dmg_sound/rom_singles/05-sweep details.gb",
    "dmg_sound/rom_singles/06-overflow on trigger.gb",
    "dmg_sound/rom_singles/07-len sweep period sync.gb",
    "dmg_sound/rom_singles/08-len ctr during power.gb",
    "dmg_sound/rom_singles/09-wave read while on.gb",
    "dmg_sound/rom_singles/10-wave trigger while on.gb",
    "dmg_sound/rom_singles/11-regs after power.gb",
    "dmg_sound/rom_singles/12-wave write while on.gb",
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
fn blargg_cpu_instrs_fixtures_present() {
    let all = fixture("cpu_instrs/cpu_instrs.gb");
    assert!(
        all.exists(),
        "missing {} — copy from retrio/gb-test-roms into tests/fixtures/blargg/",
        all.display()
    );
    for rel in CPU_INSTRS_INDIVIDUAL {
        let p = fixture(rel);
        assert!(p.exists(), "missing {}", p.display());
    }
}

#[test]
fn blargg_dmg_sound_fixtures_present() {
    let all = fixture("dmg_sound/dmg_sound.gb");
    assert!(
        all.exists(),
        "missing {} — copy from retrio/gb-test-roms into tests/fixtures/blargg/",
        all.display()
    );
    for rel in DMG_SOUND_SINGLES {
        let p = fixture(rel);
        assert!(p.exists(), "missing {}", p.display());
    }
}

#[test]
#[ignore = "Blargg cpu_instrs matrix; run with --ignored --nocapture"]
fn blargg_cpu_instrs_matrix() {
    eprintln!();
    eprintln!("{:<48} result", "ROM");
    let mut fails = Vec::new();

    for rel in CPU_INSTRS_INDIVIDUAL {
        let path = fixture(rel);
        let outcome = run_blargg(&path, STEP_LIMIT);
        print_row(rel, &outcome);
        if outcome != Outcome::Pass {
            fails.push(format!("{rel}: {} {}", outcome.label(), outcome.detail()));
        }
    }

    let all = "cpu_instrs/cpu_instrs.gb";
    let outcome = run_blargg(&fixture(all), STEP_LIMIT.saturating_mul(2));
    print_row(all, &outcome);
    if outcome != Outcome::Pass {
        fails.push(format!("{all}: {} {}", outcome.label(), outcome.detail()));
    }
    eprintln!();

    assert!(
        fails.is_empty(),
        "Blargg cpu_instrs failures:\n{}",
        fails.join("\n")
    );
}

#[test]
#[ignore = "Blargg dmg_sound matrix; run with --ignored --nocapture"]
fn blargg_dmg_sound_matrix() {
    eprintln!();
    eprintln!("{:<48} result", "ROM");
    let mut fails = Vec::new();

    for rel in DMG_SOUND_SINGLES {
        let path = fixture(rel);
        let outcome = run_blargg(&path, SOUND_STEP_LIMIT);
        print_row(rel, &outcome);
        if outcome != Outcome::Pass {
            fails.push(format!("{rel}: {} {}", outcome.label(), outcome.detail()));
        }
    }

    let all = "dmg_sound/dmg_sound.gb";
    let outcome = run_blargg(&fixture(all), SOUND_STEP_LIMIT.saturating_mul(2));
    print_row(all, &outcome);
    if outcome != Outcome::Pass {
        fails.push(format!("{all}: {} {}", outcome.label(), outcome.detail()));
    }
    eprintln!();

    assert!(
        fails.is_empty(),
        "Blargg dmg_sound failures:\n{}",
        fails.join("\n")
    );
}

#[test]
fn blargg_cgb_sound_fixtures_present() {
    let all = fixture("cgb_sound/cgb_sound.gb");
    assert!(
        all.exists(),
        "missing {} — copy from retrio/gb-test-roms into tests/fixtures/blargg/",
        all.display()
    );
    for rel in CGB_SOUND_SINGLES {
        let p = fixture(rel);
        assert!(p.exists(), "missing {}", p.display());
    }
}

#[test]
#[ignore = "Blargg cgb_sound matrix; Native CGB Fast; run with --ignored --nocapture"]
fn blargg_cgb_sound_matrix() {
    eprintln!();
    eprintln!("{:<48} result", "ROM");
    let mut fails = Vec::new();

    for rel in CGB_SOUND_SINGLES {
        let path = fixture(rel);
        let outcome = if path.exists() {
            run_blargg_launched(&path, SOUND_STEP_LIMIT, RomLaunchMode::FastCgb)
        } else {
            Outcome::Unsupported("missing fixture".into())
        };
        print_row(rel, &outcome);
        if outcome != Outcome::Pass {
            fails.push(format!("{rel}: {} {}", outcome.label(), outcome.detail()));
        }
    }

    let all = "cgb_sound/cgb_sound.gb";
    let path = fixture(all);
    let outcome = if path.exists() {
        run_blargg_launched(
            &path,
            SOUND_STEP_LIMIT.saturating_mul(2),
            RomLaunchMode::FastCgb,
        )
    } else {
        Outcome::Unsupported("missing fixture".into())
    };
    print_row(all, &outcome);
    if outcome != Outcome::Pass {
        fails.push(format!("{all}: {} {}", outcome.label(), outcome.detail()));
    }
    eprintln!();

    assert!(
        fails.is_empty(),
        "Blargg cgb_sound failures:\n{}",
        fails.join("\n")
    );
}
