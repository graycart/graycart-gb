//! MBC3 real-time clock ([Pan Docs — MBC3](https://gbdev.io/pandocs/MBC3.html)).
//!
//! Register select `$08`–`$0C` maps into `$A000`–`$BFFF`. Latch via `$6000` `$00`→`$01`.

/// Seconds per DMG CPU second (4 194 304 Hz).
pub(crate) const T_CYCLES_PER_SECOND: u64 = 4_194_304;

/// BGB / VBA-M style 48-byte RTC trailer appended after SRAM in `.sav` files.
pub(crate) const RTC_SAVE_LEN: usize = 48;

#[derive(Debug, Clone)]
pub(crate) struct Rtc {
    /// Running clock (ticks when not halted).
    s: u8,
    m: u8,
    h: u8,
    /// Day counter bits 0–7.
    dl: u8,
    /// Bit0 = day bit8, bit6 = halt, bit7 = day carry.
    dh: u8,
    /// Latched snapshot returned on RTC reads.
    latched: [u8; 5],
    /// Previous byte written to the latch port (`$6000–$7FFF`).
    latch_prev: u8,
    cycle_accum: u64,
    /// Unix seconds at last wall-clock sync (save / load).
    unix_secs: u64,
    dirty: bool,
}

impl Default for Rtc {
    fn default() -> Self {
        Self::new()
    }
}

impl Rtc {
    pub(crate) fn new() -> Self {
        Self {
            s: 0,
            m: 0,
            h: 0,
            dl: 0,
            dh: 0,
            latched: [0; 5],
            latch_prev: 0xFF,
            cycle_accum: 0,
            unix_secs: 0,
            dirty: false,
        }
    }

    pub(crate) fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub(crate) fn clear_dirty(&mut self) {
        self.dirty = false;
    }

    pub(crate) fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    pub(crate) fn halted(&self) -> bool {
        self.dh & 0x40 != 0
    }

    /// Advance from CPU T-cycles (ignored while halted).
    pub(crate) fn tick(&mut self, t_cycles: u32) {
        if self.halted() || t_cycles == 0 {
            return;
        }
        self.cycle_accum = self.cycle_accum.saturating_add(u64::from(t_cycles));
        while self.cycle_accum >= T_CYCLES_PER_SECOND {
            self.cycle_accum -= T_CYCLES_PER_SECOND;
            self.advance_seconds(1);
        }
    }

    /// Advance `secs` on the running clock (tests + save reload).
    pub(crate) fn advance_seconds(&mut self, secs: u64) {
        if self.halted() || secs == 0 {
            return;
        }
        self.mark_dirty();

        let mut days = u64::from(self.dl) | (u64::from(self.dh & 0x01) << 8);
        let mut s = u64::from(self.s) + secs;
        let mut m = u64::from(self.m);
        let mut h = u64::from(self.h);

        m += s / 60;
        s %= 60;
        h += m / 60;
        m %= 60;
        let day_add = h / 24;
        h %= 24;
        days += day_add;
        if days > 0x1FF {
            self.dh |= 0x80;
            days %= 512;
        }

        self.s = s as u8;
        self.m = m as u8;
        self.h = h as u8;
        self.dl = days as u8;
        self.dh = (self.dh & !0x01) | ((days >> 8) as u8 & 0x01);
    }

    /// Latch port write (`$6000–$7FFF`). Captures on `$00` → `$01`.
    pub(crate) fn write_latch(&mut self, value: u8) {
        let v = value & 0x01;
        if self.latch_prev == 0x00 && v == 0x01 {
            self.latched = [self.s, self.m, self.h, self.dl, self.dh];
        }
        self.latch_prev = v;
    }

    /// `reg` is `$08`–`$0C`.
    pub(crate) fn read_latched(&self, reg: u8) -> u8 {
        match reg {
            0x08 => self.latched[0],
            0x09 => self.latched[1],
            0x0A => self.latched[2],
            0x0B => self.latched[3],
            0x0C => self.latched[4],
            _ => 0xFF,
        }
    }

    /// Write running register (`$08`–`$0C`). Games should halt first.
    pub(crate) fn write_reg(&mut self, reg: u8, value: u8) {
        self.mark_dirty();
        match reg {
            0x08 => self.s = value % 60,
            0x09 => self.m = value % 60,
            0x0A => self.h = value % 24,
            0x0B => self.dl = value,
            0x0C => self.dh = value & 0xC1,
            _ => {}
        }
    }

    /// Unix timestamp used for offline catch-up on save load.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn unix_secs(&self) -> u64 {
        self.unix_secs
    }

    pub(crate) fn set_unix_secs(&mut self, secs: u64) {
        self.unix_secs = secs;
    }

    /// Apply wall-clock delta since `unix_secs`, then update the stamp.
    pub(crate) fn sync_to_unix(&mut self, now: u64) {
        if now > self.unix_secs {
            self.advance_seconds(now - self.unix_secs);
        }
        self.unix_secs = now;
    }

    /// Serialize BGB-style 48-byte trailer (5×u32 LE regs + 5×u32 latched + u64 time).
    pub(crate) fn to_save_bytes(&self) -> [u8; RTC_SAVE_LEN] {
        let mut out = [0u8; RTC_SAVE_LEN];
        let regs = [
            u32::from(self.s),
            u32::from(self.m),
            u32::from(self.h),
            u32::from(self.dl),
            u32::from(self.dh),
        ];
        let latched = self.latched.map(u32::from);
        for (i, v) in regs.into_iter().chain(latched).enumerate() {
            out[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
        }
        out[40..48].copy_from_slice(&self.unix_secs.to_le_bytes());
        out
    }

    pub(crate) fn load_save_bytes(&mut self, data: &[u8]) {
        if data.len() < RTC_SAVE_LEN {
            return;
        }
        let read_u32 =
            |off: usize| u32::from_le_bytes(data[off..off + 4].try_into().unwrap()) as u8;
        self.s = read_u32(0) % 60;
        self.m = read_u32(4) % 60;
        self.h = read_u32(8) % 24;
        self.dl = read_u32(12);
        self.dh = read_u32(16) & 0xC1;
        for i in 0..5 {
            self.latched[i] = read_u32(20 + i * 4);
        }
        self.unix_secs = u64::from_le_bytes(data[40..48].try_into().unwrap());
        self.dirty = false;
    }

    pub(crate) fn snapshot(&self) -> crate::snapshot::RtcStateV1 {
        crate::snapshot::RtcStateV1 {
            s: self.s,
            m: self.m,
            h: self.h,
            dl: self.dl,
            dh: self.dh,
            latched: self.latched,
            latch_prev: self.latch_prev,
            cycle_accum: self.cycle_accum,
            unix_secs: self.unix_secs,
        }
    }

    pub(crate) fn apply(&mut self, state: &crate::snapshot::RtcStateV1) {
        self.s = state.s;
        self.m = state.m;
        self.h = state.h;
        self.dl = state.dl;
        self.dh = state.dh;
        self.latched = state.latched;
        self.latch_prev = state.latch_prev;
        self.cycle_accum = state.cycle_accum;
        self.unix_secs = state.unix_secs;
    }
}

#[cfg(test)]
mod tests;
