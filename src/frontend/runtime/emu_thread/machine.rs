//! Owned machine instance on the emulation thread.

use graycart::{Bus, Cpu, ExecSession};
use std::path::{Path, PathBuf};

pub(super) struct Machine {
    pub path: PathBuf,
    pub save_path: PathBuf,
    pub title: String,
    pub cpu: Cpu,
    pub bus: Bus,
    pub session: ExecSession,
}

pub(super) fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
}

pub(super) fn rom_title(cart: &graycart::Cartridge, path: &Path) -> String {
    let t = cart.header.title.trim();
    if t.is_empty() {
        path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("ROM")
            .to_string()
    } else {
        t.to_string()
    }
}
