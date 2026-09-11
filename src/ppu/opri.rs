//! OBJ-to-OBJ priority mode (`$FF6C` / OPRI) on CGB silicon.
//!
//! Bit 0: `0` = CGB OAM-index order (Native Fast), `1` = DMG X-priority (Compat Fast).
//! Other bits are stored as written.

pub const OPRI: u16 = 0xFF6C;
