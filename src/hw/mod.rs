//! Console hardware model (DMG vs CGB). Launch is `resolve_launch`, not `$0143` or `.gbc`.

mod boot_map;
mod cgb_scratch;
mod clock;
mod hdma;
mod key0;
mod key1;
mod launch;

pub use boot_map::{CGB_BOOT_ROM_SIZE, cgb_boot_byte};
pub use cgb_scratch::{CgbScratch, FF72, FF73, FF74, FF75, ff74_locked};
pub use clock::{ClockState, STOP_PAUSE_CPU_T};
pub use hdma::{
    BLOCK_LEN, Block, HDMA1, HDMA2, HDMA3, HDMA4, HDMA5, Hdma5Write, StartOutcome, VramDma,
    VramDmaMode, block_cpu_t, block_fixed_t, hblank_may_transfer, mask_dest, mask_source,
    parse_hdma5_write, read_hdma5, source_is_vram,
};
pub use key0::{KEY0, Key0};
pub use key1::{KEY1, Key1};
pub use launch::bus_from_cartridge;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CgbSupportClass {
    DmgOnly,
    CgbCompatible,
    CgbOnly,
}

impl CgbSupportClass {
    pub fn from_cartridge_support(s: crate::cart::CartridgeCgbSupport) -> Self {
        use crate::cart::CartridgeCgbSupport as Cart;
        match s {
            Cart::DmgOnly => Self::DmgOnly,
            Cart::CgbCompatible => Self::CgbCompatible,
            Cart::CgbOnly => Self::CgbOnly,
            Cart::Other(b) if b & 0x80 != 0 => Self::CgbCompatible,
            Cart::Other(_) => Self::DmgOnly,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CgbExecutionMode {
    NativeCgb,
    DmgCompatibility,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HardwareModel {
    Dmg,
    Cgb { mode: CgbExecutionMode },
}

impl HardwareModel {
    /// CGB MMIO (`VBK`/`SVBK`, …) is visible on CGB silicon, including DMG-compat.
    pub fn cgb_mmio(self) -> bool {
        matches!(self, HardwareModel::Cgb { .. })
    }

    /// CGB-mode-only ports (HDMA, KEY0, KEY1). False on DMG silicon and DmgCompatibility.
    pub fn native_cgb(self) -> bool {
        matches!(
            self,
            HardwareModel::Cgb {
                mode: CgbExecutionMode::NativeCgb
            }
        )
    }

    /// Host-facing launch name (CLI / Help). Not a filename or `$0143` string.
    pub fn launch_label(self) -> &'static str {
        match self {
            HardwareModel::Dmg => "Game Boy",
            HardwareModel::Cgb {
                mode: CgbExecutionMode::NativeCgb,
            } => "Game Boy Color",
            HardwareModel::Cgb {
                mode: CgbExecutionMode::DmgCompatibility,
            } => "Game Boy Color (DMG compatibility)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostHardwarePref {
    #[default]
    Automatic,
    GameBoy,
    GameBoyColor,
}

impl HostHardwarePref {
    pub const ALL: [Self; 3] = [Self::Automatic, Self::GameBoy, Self::GameBoyColor];

    pub fn menu_label(self) -> &'static str {
        match self {
            Self::Automatic => "Automatic",
            Self::GameBoy => "Game Boy",
            Self::GameBoyColor => "Game Boy Color",
        }
    }

    pub fn cli_name(self) -> &'static str {
        match self {
            Self::Automatic => "automatic",
            Self::GameBoy => "game-boy",
            Self::GameBoyColor => "game-boy-color",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchError {
    CgbOnlyOnDmg,
}

impl std::fmt::Display for LaunchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CgbOnlyOnDmg => {
                write!(f, "CGB-only cartridge cannot run on Game Boy")
            }
        }
    }
}

impl std::error::Error for LaunchError {}

/// 10I: Automatic uses CGB silicon (DMG carts → DmgCompatibility).
pub const CGB_SILICON_READY: bool = true;

pub fn resolve_launch(
    support: CgbSupportClass,
    pref: HostHardwarePref,
    cgb_silicon_ready: bool,
) -> Result<HardwareModel, LaunchError> {
    match pref {
        HostHardwarePref::GameBoy => force_dmg_silicon(support),
        HostHardwarePref::GameBoyColor => Ok(cgb_silicon(support)),
        HostHardwarePref::Automatic => {
            if cgb_silicon_ready {
                Ok(cgb_silicon(support))
            } else {
                force_dmg_silicon(support)
            }
        }
    }
}

fn force_dmg_silicon(support: CgbSupportClass) -> Result<HardwareModel, LaunchError> {
    match support {
        CgbSupportClass::CgbOnly => Err(LaunchError::CgbOnlyOnDmg),
        CgbSupportClass::DmgOnly | CgbSupportClass::CgbCompatible => Ok(HardwareModel::Dmg),
    }
}

fn cgb_silicon(support: CgbSupportClass) -> HardwareModel {
    HardwareModel::Cgb {
        mode: match support {
            CgbSupportClass::DmgOnly => CgbExecutionMode::DmgCompatibility,
            CgbSupportClass::CgbCompatible | CgbSupportClass::CgbOnly => {
                CgbExecutionMode::NativeCgb
            }
        },
    }
}

#[cfg(test)]
mod tests;
