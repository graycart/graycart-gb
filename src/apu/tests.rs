use super::*;

#[test]
fn output_sample_rate_survives_explicit_bind_but_not_bare_power_on() {
    let mut apu = Apu::new();
    assert_eq!(apu.output_sample_rate(), HOST_SAMPLE_RATE);
    apu.set_output_sample_rate(44_100);
    assert_eq!(apu.output_sample_rate(), 44_100);
    apu.power_on_reset();
    assert_eq!(apu.output_sample_rate(), HOST_SAMPLE_RATE);
    apu.set_output_sample_rate(44_100);
    assert_eq!(apu.output_sample_rate(), 44_100);
}

#[test]
fn sample_count_uses_fractional_cycles_per_sample() {
    // 4194304 / 48000 ≈ 87.381… — integer 87 would yield ~48210 samples/sec.
    let mut apu = Apu::new();
    apu.set_output_sample_rate(48_000);
    apu.tick(CPU_HZ); // one emulated second (unpowered → silence schedule)
    let n = apu.take_samples().len();
    assert!(
        (n as i64 - 48_000).abs() <= 1,
        "expected ~48000 samples/sec with fractional phase, got {n}"
    );
}

#[test]
fn power_off_clears_channel() {
    let mut apu = Apu::new();
    apu.write(NR52, 0x80);
    apu.write(NR12, 0xF0);
    apu.write(NR14, 0x80);
    assert_eq!(apu.read(NR52).unwrap() & 0x01, 0x01);
    apu.write(NR52, 0x00);
    assert_eq!(apu.read(NR52).unwrap() & 0x80, 0);
    assert_eq!(apu.read(NR52).unwrap() & 0x01, 0);
}

#[test]
fn ticking_with_ch1_enqueues_samples() {
    let mut apu = Apu::new();
    apu.write(NR52, 0x80);
    apu.write(NR50, 0x77);
    apu.write(NR51, 0x11); // ch1 → L+R
    apu.write(NR11, 0x80);
    apu.write(NR12, 0xF3);
    apu.write(NR13, 0x00);
    apu.write(NR14, 0x87); // trigger
    apu.tick(CPU_HZ / 60); // ~one frame
    assert!(!apu.take_samples().is_empty());
}

#[test]
fn unpowered_writes_to_nr12_are_ignored() {
    let mut apu = Apu::new();
    apu.write(NR12, 0xF0);
    apu.write(NR52, 0x80);
    // After power-on, NR12 should still be 0 from power-off clear path...
    // Actually we never powered on before write — write while off is ignored.
    assert_eq!(apu.read(NR12).unwrap() & 0xF0, 0x00);
}

#[test]
fn apply_after_boot_powers_and_routes() {
    let mut apu = Apu::new();
    assert!(!apu.debug_ch1().powered);
    apu.apply_after_boot();
    let d = apu.debug_ch1();
    assert!(d.powered, "NR52 power bit after boot");
    assert_eq!(d.nr50, 0x77);
    assert_eq!(d.nr51, 0xF3);
    assert!(d.route_l && d.route_r, "boot NR51 routes CH1 both sides");
    assert_eq!(apu.read(NR52).unwrap() & 0x80, 0x80);
    // CH2 DAC off after boot.
    assert_eq!(apu.read(NR22).unwrap(), 0x00);
    assert_eq!(apu.read(NR12).unwrap(), 0xF3, "Pan Docs post-boot NR12");
    assert_eq!(apu.read(NR52).unwrap() & 0x02, 0);
    // CH3 DAC off after boot.
    assert_eq!(apu.read(NR30).unwrap() & 0x80, 0);
    assert_eq!(apu.read(NR52).unwrap() & 0x04, 0);
}

/// Synthetic Channel 1 vertical-slice: power → route → trigger → nonzero PCM.
#[test]
fn synthetic_ch1_produces_nonzero_varying_samples() {
    let mut apu = Apu::new();
    apu.write(NR52, 0x80); // power on
    apu.write(NR50, 0x77); // max master volumes
    apu.write(NR51, 0x11); // CH1 → L+R
    apu.write(NR11, 0x80); // duty 50%, length unused (NR14 length off)
    apu.write(NR12, 0xF0); // volume 15, no envelope
    apu.write(NR13, 0xD6); // ~440 Hz-ish with NR14 low bits
    apu.write(NR14, 0x86); // trigger, length disabled, freq high bits

    assert!(apu.debug_ch1().active);
    assert!(apu.debug_ch1().dac);

    // ~1/10 second of CPU time.
    apu.tick(CPU_HZ / 10);
    let samples = apu.take_samples();
    assert!(!samples.is_empty(), "expected host samples");
    assert!(
        samples.len() > 1000,
        "expected ~4800 samples @ 48kHz/10, got {}",
        samples.len()
    );

    let mut min = f32::INFINITY;
    let mut max = f32::NEG_INFINITY;
    let mut nonzero = 0usize;
    for s in &samples {
        min = min.min(s.left).min(s.right);
        max = max.max(s.left).max(s.right);
        if s.left.abs() > 0.0 || s.right.abs() > 0.0 {
            nonzero += 1;
        }
    }
    assert!(nonzero > 0, "all samples were zero — APU/mixer silent");
    assert!(
        min < -0.01 && max > 0.01,
        "expected bipolar variation, got min={min} max={max}"
    );
    assert!(
        (max - min) > 0.05,
        "samples did not vary enough: min={min} max={max}"
    );
}

/// Synthetic Channel 2: same pulse core, NR21–NR24 + NR51 bits 1/5.
#[test]
fn synthetic_ch2_produces_nonzero_varying_samples() {
    let mut apu = Apu::new();
    apu.write(NR52, 0x80);
    apu.write(NR50, 0x77);
    apu.write(NR51, 0x22); // CH2 → L+R only (CH1 silent)
    apu.write(NR21, 0x80);
    apu.write(NR22, 0xF0);
    apu.write(NR23, 0xD6);
    apu.write(NR24, 0x86); // trigger, length off

    assert_eq!(apu.read(NR52).unwrap() & 0x02, 0x02, "NR52 CH2 status");
    assert_eq!(apu.read(NR52).unwrap() & 0x01, 0, "CH1 must stay off");

    apu.tick(CPU_HZ / 10);
    let samples = apu.take_samples();
    assert!(!samples.is_empty());

    let mut min = f32::INFINITY;
    let mut max = f32::NEG_INFINITY;
    let mut nonzero = 0usize;
    let mut left_nz = 0usize;
    let mut right_nz = 0usize;
    for s in &samples {
        min = min.min(s.left).min(s.right);
        max = max.max(s.left).max(s.right);
        if s.left.abs() > 0.0 {
            left_nz += 1;
        }
        if s.right.abs() > 0.0 {
            right_nz += 1;
        }
        if s.left.abs() > 0.0 || s.right.abs() > 0.0 {
            nonzero += 1;
        }
    }
    assert!(nonzero > 0, "CH2 samples all zero");
    assert!(left_nz > 0 && right_nz > 0, "CH2 must route L and R");
    assert!(min < -0.01 && max > 0.01, "min={min} max={max}");
}

#[test]
fn ch2_left_only_routing() {
    let mut apu = Apu::new();
    apu.write(NR52, 0x80);
    apu.write(NR50, 0x77);
    apu.write(NR51, 0x20); // CH2 → L only
    apu.write(NR21, 0x80);
    apu.write(NR22, 0xF0);
    apu.write(NR23, 0x00);
    apu.write(NR24, 0x87);
    apu.tick(CPU_HZ / 60);
    let samples = apu.take_samples();
    assert!(samples.iter().any(|s| s.left.abs() > 0.01));
    assert!(samples.iter().all(|s| s.right.abs() < 0.0001));
}

#[test]
fn ch1_unchanged_when_only_ch2_configured() {
    let mut apu = Apu::new();
    apu.write(NR52, 0x80);
    apu.write(NR50, 0x77);
    apu.write(NR51, 0x11); // CH1 routes — but CH1 never triggered
    apu.write(NR21, 0x80);
    apu.write(NR22, 0xF0);
    apu.write(NR23, 0x00);
    apu.write(NR24, 0x80);
    // CH2 active but not routed → silence; CH1 inactive.
    assert_eq!(apu.read(NR52).unwrap() & 0x03, 0x02);
    apu.tick(CPU_HZ / 120);
    let samples = apu.take_samples();
    assert!(
        samples.iter().all(|s| s.left == 0.0 && s.right == 0.0),
        "unrouted CH2 must not leak into mixer"
    );
}

fn load_triangle_wave(apu: &mut Apu) {
    // Rising then falling 4-bit ramp across 32 nibbles (16 bytes).
    let bytes: [u8; 16] = [
        0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF, // 0..15
        0xFE, 0xDC, 0xBA, 0x98, 0x76, 0x54, 0x32, 0x10, // 15..0
    ];
    for (i, b) in bytes.iter().enumerate() {
        apu.write(WAVE_RAM_START + i as u16, *b);
    }
}

#[test]
fn wave_ram_round_trip_while_apu_off() {
    let mut apu = Apu::new();
    apu.write(WAVE_RAM_START, 0xAB);
    assert_eq!(apu.read(WAVE_RAM_START).unwrap(), 0xAB);
    assert_eq!(apu.read(NR52).unwrap() & 0x80, 0);
}

#[test]
fn synthetic_ch3_produces_nonzero_varying_samples() {
    let mut apu = Apu::new();
    load_triangle_wave(&mut apu);
    apu.write(NR52, 0x80);
    apu.write(NR50, 0x77);
    apu.write(NR51, 0x44); // CH3 → L+R only
    apu.write(NR30, 0x80); // DAC on
    apu.write(NR32, 0x20); // 100%
    apu.write(NR33, 0x00);
    apu.write(NR34, 0x87); // trigger, length off, high freq

    assert_eq!(apu.read(NR52).unwrap() & 0x04, 0x04);
    assert_eq!(apu.read(NR52).unwrap() & 0x03, 0, "pulse channels stay off");

    apu.tick(CPU_HZ / 10);
    let samples = apu.take_samples();
    assert!(!samples.is_empty());

    let mut min = f32::INFINITY;
    let mut max = f32::NEG_INFINITY;
    let mut nonzero = 0usize;
    for s in &samples {
        min = min.min(s.left).min(s.right);
        max = max.max(s.left).max(s.right);
        if s.left.abs() > 0.01 || s.right.abs() > 0.01 {
            nonzero += 1;
        }
    }
    assert!(nonzero > 0, "CH3 silent");
    assert!(
        min < -0.05 && max > 0.05,
        "expected wave variation min={min} max={max}"
    );
}

#[test]
fn ch3_output_level_mute_is_silent() {
    let mut apu = Apu::new();
    load_triangle_wave(&mut apu);
    apu.write(NR52, 0x80);
    apu.write(NR50, 0x77);
    apu.write(NR51, 0x44);
    apu.write(NR30, 0x80);
    apu.write(NR32, 0x00); // mute
    apu.write(NR33, 0x00);
    apu.write(NR34, 0x87);
    apu.tick(CPU_HZ / 60);
    let samples = apu.take_samples();
    assert!(samples.iter().all(|s| s.left == 0.0 && s.right == 0.0));
}

#[test]
fn ch3_dac_off_kills_channel() {
    let mut apu = Apu::new();
    load_triangle_wave(&mut apu);
    apu.write(NR52, 0x80);
    apu.write(NR50, 0x77);
    apu.write(NR51, 0x44);
    apu.write(NR30, 0x80);
    apu.write(NR32, 0x20);
    apu.write(NR34, 0x80);
    assert_eq!(apu.read(NR52).unwrap() & 0x04, 0x04);
    apu.write(NR30, 0x00);
    assert_eq!(apu.read(NR52).unwrap() & 0x04, 0);
}

#[test]
fn ch3_left_only_routing() {
    let mut apu = Apu::new();
    load_triangle_wave(&mut apu);
    apu.write(NR52, 0x80);
    apu.write(NR50, 0x77);
    apu.write(NR51, 0x40); // CH3 → L only
    apu.write(NR30, 0x80);
    apu.write(NR32, 0x20);
    apu.write(NR33, 0x00);
    apu.write(NR34, 0x87);
    apu.tick(CPU_HZ / 60);
    let samples = apu.take_samples();
    assert!(samples.iter().any(|s| s.left.abs() > 0.01));
    assert!(samples.iter().all(|s| s.right.abs() < 0.0001));
}

#[test]
fn ch3_does_not_break_ch1() {
    let mut apu = Apu::new();
    load_triangle_wave(&mut apu);
    apu.write(NR52, 0x80);
    apu.write(NR50, 0x77);
    apu.write(NR51, 0x11); // CH1 only
    apu.write(NR30, 0x80);
    apu.write(NR32, 0x20);
    apu.write(NR34, 0x80); // CH3 on but not routed
    apu.write(NR11, 0x80);
    apu.write(NR12, 0xF0);
    apu.write(NR13, 0x00);
    apu.write(NR14, 0x87);
    apu.tick(CPU_HZ / 60);
    let samples = apu.take_samples();
    assert!(samples.iter().any(|s| s.left.abs() > 0.01));
    assert_eq!(apu.read(NR52).unwrap() & 0x05, 0x05); // CH1 + CH3 active
}

#[test]
fn apu_power_off_preserves_wave_ram() {
    let mut apu = Apu::new();
    apu.write(WAVE_RAM_START, 0x5A);
    apu.write(NR52, 0x80);
    apu.write(NR30, 0x80);
    apu.write(NR52, 0x00);
    assert_eq!(apu.read(WAVE_RAM_START).unwrap(), 0x5A);
}

#[test]
fn apply_after_boot_ch4_dac_off() {
    let mut apu = Apu::new();
    apu.apply_after_boot();
    assert_eq!(apu.read(NR42).unwrap(), 0x00);
    assert_eq!(apu.read(NR52).unwrap() & 0x08, 0);
}

#[test]
fn synthetic_ch4_produces_nonzero_varying_samples() {
    let mut apu = Apu::new();
    apu.write(NR52, 0x80);
    apu.write(NR50, 0x77);
    apu.write(NR51, 0x88); // CH4 → L+R only
    apu.write(NR42, 0xF0); // max volume, DAC on, no envelope
    apu.write(NR43, 0x00); // fast LFSR
    apu.write(NR44, 0x80); // trigger

    assert_eq!(apu.read(NR52).unwrap() & 0x08, 0x08);
    assert_eq!(apu.read(NR52).unwrap() & 0x07, 0);

    apu.tick(CPU_HZ / 10);
    let samples = apu.take_samples();
    assert!(!samples.is_empty());

    let mut min = f32::INFINITY;
    let mut max = f32::NEG_INFINITY;
    let mut nonzero = 0usize;
    for s in &samples {
        min = min.min(s.left).min(s.right);
        max = max.max(s.left).max(s.right);
        if s.left.abs() > 0.01 || s.right.abs() > 0.01 {
            nonzero += 1;
        }
    }
    assert!(nonzero > 0, "CH4 silent");
    assert!(
        min < -0.05 && max > 0.05,
        "expected noise variation min={min} max={max}"
    );
}

#[test]
fn ch4_dac_off_kills_channel() {
    let mut apu = Apu::new();
    apu.write(NR52, 0x80);
    apu.write(NR42, 0xF0);
    apu.write(NR44, 0x80);
    assert_eq!(apu.read(NR52).unwrap() & 0x08, 0x08);
    apu.write(NR42, 0x07);
    assert_eq!(apu.read(NR52).unwrap() & 0x08, 0);
}

#[test]
fn ch4_left_only_routing() {
    let mut apu = Apu::new();
    apu.write(NR52, 0x80);
    apu.write(NR50, 0x77);
    apu.write(NR51, 0x80); // CH4 → L only
    apu.write(NR42, 0xF0);
    apu.write(NR43, 0x00);
    apu.write(NR44, 0x80);
    apu.tick(CPU_HZ / 60);
    let samples = apu.take_samples();
    assert!(samples.iter().any(|s| s.left.abs() > 0.01));
    assert!(samples.iter().all(|s| s.right.abs() < 0.0001));
}

#[test]
fn ch4_does_not_break_lower_channels() {
    let mut apu = Apu::new();
    apu.write(NR52, 0x80);
    apu.write(NR50, 0x77);
    apu.write(NR51, 0x11); // CH1 only
    apu.write(NR42, 0xF0);
    apu.write(NR43, 0x00);
    apu.write(NR44, 0x80); // CH4 on, unrouted
    apu.write(NR11, 0x80);
    apu.write(NR12, 0xF0);
    apu.write(NR13, 0x00);
    apu.write(NR14, 0x87);
    apu.tick(CPU_HZ / 60);
    let samples = apu.take_samples();
    assert!(samples.iter().any(|s| s.left.abs() > 0.01));
    assert_eq!(apu.read(NR52).unwrap() & 0x09, 0x09); // CH1 + CH4
}
