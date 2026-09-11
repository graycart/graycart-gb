//! Battery-backed save RAM (`.sav`) — filesystem I/O outside the MBC.
//!
//! Cartridge / MBC owns the RAM buffer, banking, and RTC; this module only
//! loads and flushes the save image (SRAM + optional 48-byte RTC trailer).

use crate::cart::Cartridge;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Derive `foo.sav` from `foo.gb` / `foo.gbc`, otherwise append `.sav`.
pub fn default_save_path(rom_path: impl AsRef<Path>) -> PathBuf {
    let path = rom_path.as_ref();
    match path.extension().and_then(|e| e.to_str()) {
        Some(ext) if ext.eq_ignore_ascii_case("gb") || ext.eq_ignore_ascii_case("gbc") => {
            path.with_extension("sav")
        }
        _ => {
            let mut os = path.as_os_str().to_owned();
            os.push(".sav");
            PathBuf::from(os)
        }
    }
}

/// Load a `.sav` into battery SRAM / RTC if the file exists.
///
/// Returns `true` when bytes were applied. Missing files and carts without
/// persistent state are not errors (`Ok(false)`).
pub fn load(path: impl AsRef<Path>, cart: &mut Cartridge) -> io::Result<bool> {
    if !cart.needs_save() {
        return Ok(false);
    }
    let path = path.as_ref();
    if !path.exists() {
        return Ok(false);
    }
    let data = fs::read(path)?;
    cart.load_save_image(&data);
    Ok(true)
}

/// Write dirty save image to `path`. No-op when nothing to persist or clean.
///
/// Returns `true` when a file was written.
pub fn flush(path: impl AsRef<Path>, cart: &mut Cartridge) -> io::Result<bool> {
    if !cart.needs_save() || !cart.is_save_dirty() {
        return Ok(false);
    }
    fs::write(path.as_ref(), cart.save_image())?;
    cart.clear_save_dirty();
    Ok(true)
}

#[cfg(test)]
mod tests;
