//! ROM open / recent list helpers (host filesystem only).

mod recent;

#[cfg(test)]
mod tests;

pub use recent::RecentRom;

use graycart::{Cartridge, default_save_path, legacy_sidecar_save_path, load_save_with_fallback};
use std::path::{Path, PathBuf};

/// Supported cartridge file extensions (no leading dot; case-insensitive at match time).
pub const ROM_EXTENSIONS: &[&str] = &["gb", "gbc", "rom", "bin"];

pub struct OpenedRom {
    pub path: PathBuf,
    pub save_path: PathBuf,
    pub title: String,
    pub cart: Cartridge,
}

pub fn open_rom_file(path: &Path) -> Result<OpenedRom, String> {
    let mut cart =
        Cartridge::load(path).map_err(|e| format!("failed to load {}: {e}", path.display()))?;
    if !cart.header_ok() {
        return Err("invalid cartridge header (logo or header checksum failed)".into());
    }
    let title = {
        let t = cart.header.title.trim();
        if t.is_empty() {
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Unknown")
                .to_string()
        } else {
            t.to_string()
        }
    };
    let save_path = default_save_path(path);
    let legacy = legacy_sidecar_save_path(path);
    let _ = load_save_with_fallback(&save_path, Some(&legacy), &mut cart);
    Ok(OpenedRom {
        path: path.to_path_buf(),
        save_path,
        title,
        cart,
    })
}

pub fn is_rom_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| {
            ROM_EXTENSIONS
                .iter()
                .any(|known| ext.eq_ignore_ascii_case(known))
        })
}

/// File-dialog filter extensions for native ROM pickers (`rfd`, etc.).
pub fn rom_dialog_extensions() -> &'static [&'static str] {
    ROM_EXTENSIONS
}
