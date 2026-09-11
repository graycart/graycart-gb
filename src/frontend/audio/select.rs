//! Output-device selection: user pref → OS/CPAL default → explicit fallback.

/// Persisted output preference. Fallback devices are never stored here.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AudioDevicePref {
    #[default]
    SystemDefault,
    Device {
        name: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioDeviceSource {
    User,
    SystemDefault,
    Fallback,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioOutputChoice {
    Device {
        name: String,
        source: AudioDeviceSource,
    },
    Offline,
}

/// Pure selection (no CPAL). `os_defaults` are native endpoint names in
/// preference order (WASAPI / Core Audio / Pulse+ALSA bridges).
pub fn choose_output_device(
    pref: &AudioDevicePref,
    cpal_default: Option<&str>,
    os_defaults: &[&str],
    enumerated: &[&str],
) -> AudioOutputChoice {
    match pref {
        AudioDevicePref::Device { name } => {
            if enumerated.contains(&name.as_str()) {
                return AudioOutputChoice::Device {
                    name: name.clone(),
                    source: AudioDeviceSource::User,
                };
            }
            // Missing user device: fall through to SystemDefault, do not keep using
            // an arbitrary enumeration order as if the user chose it.
            choose_system_default(cpal_default, os_defaults, enumerated)
        }
        AudioDevicePref::SystemDefault => {
            choose_system_default(cpal_default, os_defaults, enumerated)
        }
    }
}

fn choose_system_default(
    cpal_default: Option<&str>,
    os_defaults: &[&str],
    enumerated: &[&str],
) -> AudioOutputChoice {
    for name in os_defaults {
        if enumerated.contains(name) {
            return AudioOutputChoice::Device {
                name: (*name).to_string(),
                source: AudioDeviceSource::SystemDefault,
            };
        }
    }
    if let Some(name) = cpal_default
        && enumerated.contains(&name)
    {
        return AudioOutputChoice::Device {
            name: name.to_string(),
            source: AudioDeviceSource::SystemDefault,
        };
    }
    if let Some(name) = cpal_default {
        return AudioOutputChoice::Device {
            name: name.to_string(),
            source: AudioDeviceSource::SystemDefault,
        };
    }
    match enumerated.first() {
        Some(name) => AudioOutputChoice::Device {
            name: (*name).to_string(),
            source: AudioDeviceSource::Fallback,
        },
        None => AudioOutputChoice::Offline,
    }
}

pub fn should_persist_choice(source: AudioDeviceSource) -> bool {
    matches!(source, AudioDeviceSource::User)
}

/// Menu listing must not re-enumerate WASAPI/CPAL every egui frame.
pub fn should_refresh_output_device_list(
    age: Option<std::time::Duration>,
    ttl: std::time::Duration,
) -> bool {
    age.is_none_or(|a| a >= ttl)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_default_prefers_os_endpoint_not_first_enumerated() {
        let choice = choose_output_device(
            &AudioDevicePref::SystemDefault,
            None,
            &["Speakers (Realtek)"],
            &["Digital Output (Sound Blaster X4)", "Speakers (Realtek)"],
        );
        assert_eq!(
            choice,
            AudioOutputChoice::Device {
                name: "Speakers (Realtek)".into(),
                source: AudioDeviceSource::SystemDefault,
            }
        );
    }

    #[test]
    fn system_default_uses_cpal_default_over_enumeration_order() {
        let choice = choose_output_device(
            &AudioDevicePref::SystemDefault,
            Some("Speakers (Realtek)"),
            &[],
            &["Digital Output (Sound Blaster X4)", "Speakers (Realtek)"],
        );
        assert_eq!(
            choice,
            AudioOutputChoice::Device {
                name: "Speakers (Realtek)".into(),
                source: AudioDeviceSource::SystemDefault,
            }
        );
    }

    #[test]
    fn missing_defaults_use_fallback_without_calling_it_user_selected() {
        let choice = choose_output_device(
            &AudioDevicePref::SystemDefault,
            None,
            &[],
            &["Digital Output (Sound Blaster X4)"],
        );
        assert_eq!(
            choice,
            AudioOutputChoice::Device {
                name: "Digital Output (Sound Blaster X4)".into(),
                source: AudioDeviceSource::Fallback,
            }
        );
        assert!(!should_persist_choice(AudioDeviceSource::Fallback));
    }

    #[test]
    fn user_named_device_wins_when_present() {
        let pref = AudioDevicePref::Device {
            name: "Digital Output (Sound Blaster X4)".into(),
        };
        let choice = choose_output_device(
            &pref,
            Some("Speakers (Realtek)"),
            &["Speakers (Realtek)"],
            &["Digital Output (Sound Blaster X4)", "Speakers (Realtek)"],
        );
        assert_eq!(
            choice,
            AudioOutputChoice::Device {
                name: "Digital Output (Sound Blaster X4)".into(),
                source: AudioDeviceSource::User,
            }
        );
        assert!(should_persist_choice(AudioDeviceSource::User));
    }

    #[test]
    fn missing_user_device_falls_back_to_system_default() {
        let pref = AudioDevicePref::Device {
            name: "Gone Device".into(),
        };
        let choice = choose_output_device(
            &pref,
            Some("Speakers (Realtek)"),
            &[],
            &["Speakers (Realtek)", "Headphones"],
        );
        assert_eq!(
            choice,
            AudioOutputChoice::Device {
                name: "Speakers (Realtek)".into(),
                source: AudioDeviceSource::SystemDefault,
            }
        );
    }

    #[test]
    fn empty_inventory_is_offline() {
        assert_eq!(
            choose_output_device(&AudioDevicePref::SystemDefault, None, &[], &[]),
            AudioOutputChoice::Offline
        );
    }

    #[test]
    fn linux_pulse_bridge_beats_first_alsa_hw_card() {
        let choice = choose_output_device(
            &AudioDevicePref::SystemDefault,
            Some("default"),
            &[
                "alsa_output.pci-0000_00_1f.3.analog-stereo",
                "pulse",
                "pipewire",
            ],
            &["hw:0,0", "default", "pulse", "pipewire"],
        );
        assert_eq!(
            choice,
            AudioOutputChoice::Device {
                name: "pulse".into(),
                source: AudioDeviceSource::SystemDefault,
            }
        );
    }

    #[test]
    fn macos_coreaudio_name_beats_first_enumerated() {
        let choice = choose_output_device(
            &AudioDevicePref::SystemDefault,
            None,
            &["MacBook Pro Speakers"],
            &["USB Audio Device", "MacBook Pro Speakers"],
        );
        assert_eq!(
            choice,
            AudioOutputChoice::Device {
                name: "MacBook Pro Speakers".into(),
                source: AudioDeviceSource::SystemDefault,
            }
        );
    }

    #[test]
    fn device_list_refresh_only_when_missing_or_ttl_elapsed() {
        let ttl = std::time::Duration::from_secs(2);
        assert!(should_refresh_output_device_list(None, ttl));
        assert!(!should_refresh_output_device_list(
            Some(std::time::Duration::from_millis(10)),
            ttl
        ));
        assert!(should_refresh_output_device_list(
            Some(std::time::Duration::from_secs(2)),
            ttl
        ));
    }
}
