//! Windows release console policy for the `graycart` binary.
//!
//! Release builds use the GUI subsystem (`windows_subsystem = "windows"`) so an
//! Explorer double-click does not open a companion console. Debug builds keep the
//! default console subsystem for ordinary `cargo run` diagnostics.
//!
//! GUI-subsystem processes start detached from any console, so CLI/headless paths
//! (`--frames`, `--version`, traces, …) would otherwise silently drop stdout.
//! [`attach_parent_console`] reconnects to the launching terminal when one exists.

/// Attach to the parent process console when present (no-op if none).
///
/// Call once at the start of `main` on Windows release builds, before any
/// `print!` / `eprintln!`. Failure is expected for Explorer launches and is
/// ignored — that is the double-click path that must stay console-free.
#[cfg(all(windows, not(debug_assertions)))]
pub fn attach_parent_console() {
    use windows::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};

    // SAFETY: AttachConsole only takes a process id; ERROR_INVALID_HANDLE /
    // ERROR_ACCESS_DENIED when there is no parent console is the Explorer case.
    unsafe {
        let _ = AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn release_main_requests_windows_gui_subsystem() {
        let main = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/main.rs"));
        assert!(
            main.contains("windows_subsystem = \"windows\""),
            "Windows release builds must mark the binary as a GUI-subsystem app \
             so Explorer launches do not open a console window"
        );
        assert!(
            main.contains("not(debug_assertions)"),
            "GUI subsystem must stay release-only so debug `cargo run` keeps a console"
        );
    }
}
