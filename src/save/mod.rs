//! Battery-backed save RAM (`.sav`) — filesystem I/O outside the MBC.
//!
//! Cartridge / MBC owns the RAM buffer, banking, and RTC; this module only
//! loads and flushes the save image (SRAM + optional 48-byte RTC trailer).
//!
//! # Path policy
//!
//! Default battery paths live under the platform Graycart data directory
//! (`…/Graycart/saves/{stem}.sav`). Existing ROM-adjacent `.sav` files are
//! still loaded as a one-shot migration fallback; flushes always go to the
//! preferred path (no dual-write). `--save <path>` remains an explicit override.

use crate::cart::Cartridge;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Platform Graycart `saves/` directory (`dirs::data_dir()/Graycart/saves`).
///
/// When no data directory is available, falls back to a repo-/cwd-relative
/// `saves/` folder (handy for smoke workflows).
pub fn graycart_saves_dir() -> PathBuf {
    if let Some(mut dir) = dirs::data_dir() {
        dir.push("Graycart");
        dir.push("saves");
        dir
    } else {
        PathBuf::from("saves")
    }
}

/// `{stem}.sav` under `dir` for the given ROM path.
pub fn save_path_in_dir(dir: impl AsRef<Path>, rom_path: impl AsRef<Path>) -> PathBuf {
    let rom = rom_path.as_ref();
    let stem = rom
        .file_stem()
        .map(OsString::from)
        .unwrap_or_else(|| OsString::from("cart"));
    let mut name = stem;
    name.push(".sav");
    dir.as_ref().join(name)
}

/// Preferred battery save path under [`graycart_saves_dir`].
pub fn default_save_path(rom_path: impl AsRef<Path>) -> PathBuf {
    save_path_in_dir(graycart_saves_dir(), rom_path)
}

/// Historical ROM-adjacent `.sav` path (load-only migration fallback).
///
/// `foo.gb` / `foo.gbc` → `foo.sav`; otherwise append `.sav`.
pub fn legacy_sidecar_save_path(rom_path: impl AsRef<Path>) -> PathBuf {
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

/// Load from `primary`, or from `legacy` when the primary file is missing.
///
/// Used for default path policy so ROM-adjacent batteries are not stranded.
/// Pass `legacy: None` when an explicit `--save` override is in effect.
pub fn load_with_fallback(
    primary: impl AsRef<Path>,
    legacy: Option<&Path>,
    cart: &mut Cartridge,
) -> io::Result<bool> {
    if load(primary.as_ref(), cart)? {
        return Ok(true);
    }
    if let Some(legacy) = legacy
        && legacy != primary.as_ref()
    {
        return load(legacy, cart);
    }
    Ok(false)
}

/// Write dirty save image to `path`. No-op when nothing to persist or clean.
///
/// Creates parent directories when needed. Returns `true` when a file was written.
pub fn flush(path: impl AsRef<Path>, cart: &mut Cartridge) -> io::Result<bool> {
    if !cart.needs_save() || !cart.is_save_dirty() {
        return Ok(false);
    }
    let path = path.as_ref();
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, cart.save_image())?;
    cart.clear_save_dirty();
    Ok(true)
}

#[cfg(test)]
mod tests;
