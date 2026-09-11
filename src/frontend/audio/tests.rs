use super::{bind_bus_host_rate, probe_audio_backends};
use graycart::{Cartridge, HOST_SAMPLE_RATE, HostHardwarePref, bus_from_cartridge};

fn tiny_rom() -> Vec<u8> {
    vec![0u8; 0x200]
}

#[test]
fn probe_audio_backends_names_hosts() {
    let dump = probe_audio_backends();
    assert!(
        dump.contains("audio backend probe:"),
        "probe dump should always start with a header, got {dump}"
    );
    assert!(
        dump.contains("available hosts:") || dump.contains("default host:"),
        "{dump}"
    );
}

#[test]
fn bind_host_rate_after_rom_load_reset_and_hardware_relaunch() {
    let cart = Cartridge::rom_only(tiny_rom());
    let mut bus = bus_from_cartridge(cart, HostHardwarePref::GameBoy).expect("dmg bus");
    bind_bus_host_rate(&mut bus, Some(44_100));
    assert_eq!(bus.apu.output_sample_rate(), 44_100);

    bus.power_on_keep_battery();
    assert_eq!(
        bus.apu.output_sample_rate(),
        HOST_SAMPLE_RATE,
        "power-on rebuilds the APU at the default host-rate constant"
    );
    bind_bus_host_rate(&mut bus, Some(44_100));
    assert_eq!(bus.apu.output_sample_rate(), 44_100);

    let cart = Cartridge::rom_only(tiny_rom());
    let mut cgb = bus_from_cartridge(cart, HostHardwarePref::GameBoyColor).expect("cgb bus");
    bind_bus_host_rate(&mut cgb, Some(48_000));
    assert_eq!(cgb.apu.output_sample_rate(), 48_000);
    cgb.power_on_keep_battery();
    bind_bus_host_rate(&mut cgb, Some(44_100));
    assert_eq!(cgb.apu.output_sample_rate(), 44_100);

    bind_bus_host_rate(&mut cgb, None);
    assert_eq!(
        cgb.apu.output_sample_rate(),
        44_100,
        "missing host audio must not clobber a previously bound rate"
    );
}
