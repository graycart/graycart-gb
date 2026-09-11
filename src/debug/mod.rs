//! Emulator diagnostics: fault reports, instruction history, and trace formatting.
//!
//! Kept out of CPU/PPU/main orchestration so crash dumps stay one place.

mod fault;
mod history;
mod machine;
pub(crate) mod profile;
mod session;
mod trace;

pub use fault::{CpuFault, FaultReport, RunOutcome};
pub use history::{HISTORY_LEN, HistoryEntry, InstructionHistory};
pub use machine::{
    ApuDebug, ChannelDebug, CpuDebug, InputDebug, InterruptDebug, MachineDebug, PpuDebug,
    TimerDebug,
};
pub use profile::TickProfile;
pub use session::ExecSession;
pub use trace::{format_bytes, format_trace_line};
