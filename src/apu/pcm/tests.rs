use super::{PCM12, PCM34, pack_pcm};
use crate::apu::{Apu, NR11, NR12, NR14, NR21, NR22, NR24, NR52};

#[test]
fn pcm_mmio_addresses() {
    assert_eq!(PCM12, 0xFF76);
    assert_eq!(PCM34, 0xFF77);
}

#[test]
fn pack_pcm_places_lo_and_hi_nibbles() {
    assert_eq!(pack_pcm(0x0A, 0x0B), 0xBA);
    assert_eq!(pack_pcm(0x1F, 0x20), 0x0F);
}

#[test]
fn apu_read_does_not_claim_pcm_ports() {
    let apu = Apu::new();
    assert_eq!(apu.read(PCM12), None);
    assert_eq!(apu.read(PCM34), None);
}

#[test]
fn silent_or_disabled_channels_read_pcm_zero() {
    let apu = Apu::new();
    assert_eq!(apu.pcm12(), 0x00);
    assert_eq!(apu.pcm34(), 0x00);

    let mut powered = Apu::new();
    powered.write(NR52, 0x80);
    assert_eq!(powered.pcm12(), 0x00);
    assert_eq!(powered.pcm34(), 0x00);
}

#[test]
fn triggered_pulse_digital_nibble_is_volume_when_duty_high() {
    let mut apu = Apu::new();
    apu.write(NR52, 0x80);
    // 25% duty: first step is 1 → digital = envelope volume ($F).
    apu.write(NR11, 0x40);
    apu.write(NR12, 0xF0);
    apu.write(NR14, 0x80);
    assert_eq!(apu.pcm12() & 0x0F, 0x0F);

    apu.write(NR21, 0x40);
    apu.write(NR22, 0x80);
    apu.write(NR24, 0x80);
    assert_eq!(apu.pcm12(), 0x8F);

    apu.write(NR52, 0x00);
    assert_eq!(apu.pcm12(), 0x00, "APU power-off forces digital 0");
    assert_eq!(apu.pcm34(), 0x00);
}
