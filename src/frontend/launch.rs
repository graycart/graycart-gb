//! Shared launch / verbosity types for windowed and headless frontends.

use graycart::Cartridge;
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Verbosity {
    Quiet,
    Normal,
    Verbose,
}

/// Optional initial ROM for windowed play (`None` → empty launcher).
pub struct LaunchRom {
    pub path: PathBuf,
    pub save_path: PathBuf,
    pub cart: Cartridge,
}

/// Cold-launch session must look attached without a second File→Open.
pub fn cold_launch_rom_attached(initial: Option<&LaunchRom>) -> bool {
    initial.is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_launcher_is_not_an_attached_session() {
        assert!(!cold_launch_rom_attached(None));
    }

    #[test]
    fn cli_rom_attaches_without_a_second_load() {
        let launch = LaunchRom {
            path: PathBuf::from("cold.gb"),
            save_path: PathBuf::from("cold.sav"),
            cart: Cartridge::rom_only(vec![0u8; 0x200]),
        };
        assert!(cold_launch_rom_attached(Some(&launch)));
    }
}
