//! Cartridge → `Bus` using [`super::resolve_launch`]. Not a filename/`.gbc` selector.

use super::{CGB_SILICON_READY, CgbSupportClass, HostHardwarePref, LaunchError, resolve_launch};
use crate::bus::Bus;
use crate::cart::Cartridge;

pub fn bus_from_cartridge(cart: Cartridge, pref: HostHardwarePref) -> Result<Bus, LaunchError> {
    let support = CgbSupportClass::from_cartridge_support(cart.header.cgb);
    let model = resolve_launch(support, pref, CGB_SILICON_READY)?;
    Ok(Bus::new(cart).with_hardware_model(model))
}
