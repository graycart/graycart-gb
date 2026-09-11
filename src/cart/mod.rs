mod cart_type;
mod destination;
mod header;
mod licensee;
mod mbc;
mod ram_size;
mod rom_size;

pub use cart_type::CartType;
pub use destination::Destination;
pub use header::{CartridgeCgbSupport, CgbFlag, Header, HeaderError};
pub use ram_size::RamSize;
pub use rom_size::RomSize;

use mbc::{MBC2_RAM_SIZE, Mapper, RAM_BANK_SIZE, ROM_BANK_SIZE, RTC_SAVE_LEN};
use std::fs;
use std::io;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Canonical Nintendo logo bitmap required by the boot ROM (0x0104..0x0133).
pub const NINTENDO_LOGO: [u8; 48] = [
    0xCE, 0xED, 0x66, 0x66, 0xCC, 0x0D, 0x00, 0x0B, 0x03, 0x73, 0x00, 0x83, 0x00, 0x0C, 0x00, 0x0D,
    0x00, 0x08, 0x11, 0x1F, 0x88, 0x89, 0x00, 0x0E, 0xDC, 0xCC, 0x6E, 0xE6, 0xDD, 0xDD, 0xD9, 0x99,
    0xBB, 0xBB, 0x67, 0x63, 0x6E, 0x0E, 0xEC, 0xCC, 0xDD, 0xDC, 0x99, 0x9F, 0xBB, 0xB9, 0x33, 0x3E,
];

/// Old licensee value that means the new licensee code at 0x0144–0x0145 is used.
pub const OLD_LICENSEE_USE_NEW: u8 = 0x33;

/// SGB flag value that enables Super Game Boy functions.
pub const SGB_FLAG_ENABLED: u8 = 0x03;

/// Fallback label for header fields we don't recognize yet.
const UNKNOWN: &str = "unknown (see Pan Docs)";

/// Loaded ROM image, parsed header, optional external RAM, and bank controller.
pub struct Cartridge {
    pub rom: Vec<u8>,
    pub header: Header,
    ram: Vec<u8>,
    mapper: Mapper,
    /// Set when battery-backed external RAM is written (for `.sav` flush).
    ram_dirty: bool,
}

impl Cartridge {
    /// Read a .gb/.gbc file and parse the cartridge header.
    pub fn load<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let rom = fs::read(path)?;
        let header =
            Header::parse(&rom).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        Self::from_parts(rom, header)
    }

    /// ROM-only cartridge for tests (flat mapping, no external RAM / MBC).
    pub fn rom_only(mut rom: Vec<u8>) -> Self {
        if rom.len() < 0x150 {
            rom.resize(0x150, 0);
        }
        rom[0x0147] = 0x00; // ROM ONLY
        rom[0x0148] = 0x00; // 32 KiB
        rom[0x0149] = 0x00; // no RAM
        let header = Header::parse(&rom).expect("rom_only image must parse");
        Self::from_parts(rom, header).expect("ROM ONLY is always supported")
    }

    pub(crate) fn from_parts(rom: Vec<u8>, header: Header) -> io::Result<Self> {
        let mapper = Mapper::try_for_cart_type(header.cart_type, header.cart_type_byte)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let ram_bytes = ram_size_bytes(&header);
        Ok(Self {
            rom,
            header,
            ram: vec![0; ram_bytes],
            mapper,
            ram_dirty: false,
        })
    }

    /// True when this cart has battery-backed external RAM worth persisting.
    pub fn has_battery_backed_ram(&self) -> bool {
        self.header.cart_type.is_some_and(CartType::has_battery) && !self.ram.is_empty()
    }

    /// True when this cart includes an MBC3 real-time clock.
    pub fn has_rtc(&self) -> bool {
        self.header.cart_type.is_some_and(CartType::has_rtc)
    }

    /// Battery SRAM and/or RTC that should participate in `.sav` I/O.
    pub fn needs_save(&self) -> bool {
        self.has_battery_backed_ram() || self.has_rtc()
    }

    /// External cartridge RAM image (SRAM), length from the header RAM size.
    pub fn external_ram(&self) -> &[u8] {
        &self.ram
    }

    /// Whether external RAM has been written since last load/clear.
    pub fn is_ram_dirty(&self) -> bool {
        self.ram_dirty
    }

    pub fn is_rtc_dirty(&self) -> bool {
        match &self.mapper {
            Mapper::Mbc3(m) => m.rtc.is_dirty(),
            _ => false,
        }
    }

    /// SRAM or RTC changed since last successful flush/load.
    pub fn is_save_dirty(&self) -> bool {
        self.ram_dirty || self.is_rtc_dirty()
    }

    pub fn clear_ram_dirty(&mut self) {
        self.ram_dirty = false;
    }

    pub fn clear_save_dirty(&mut self) {
        self.ram_dirty = false;
        if let Some(rtc) = self.mapper.mbc3_rtc_mut() {
            rtc.clear_dirty();
        }
    }

    /// Replace external RAM from a `.sav` (truncates or zero-pads to cart size).
    pub fn load_external_ram(&mut self, data: &[u8]) {
        if self.ram.is_empty() {
            return;
        }
        let n = data.len().min(self.ram.len());
        self.ram[..n].copy_from_slice(&data[..n]);
        if n < self.ram.len() {
            self.ram[n..].fill(0);
        }
        self.ram_dirty = false;
    }

    /// Load SRAM (+ optional 48-byte RTC trailer) from a save image.
    pub fn load_save_image(&mut self, data: &[u8]) {
        let ram_len = self.ram.len();
        if ram_len > 0 {
            let ram_part = if data.len() >= ram_len {
                &data[..ram_len]
            } else {
                data
            };
            self.load_external_ram(ram_part);
        }
        if self.has_rtc() {
            let now = unix_now();
            if let Some(rtc) = self.mapper.mbc3_rtc_mut() {
                if data.len() >= ram_len + RTC_SAVE_LEN {
                    rtc.load_save_bytes(&data[ram_len..ram_len + RTC_SAVE_LEN]);
                    rtc.sync_to_unix(now);
                } else {
                    rtc.set_unix_secs(now);
                }
                rtc.clear_dirty();
            }
        }
    }

    /// Build `.sav` bytes: SRAM followed by RTC trailer when present.
    pub fn save_image(&mut self) -> Vec<u8> {
        let mut out = self.ram.clone();
        if self.has_rtc()
            && let Some(rtc) = self.mapper.mbc3_rtc_mut()
        {
            rtc.set_unix_secs(unix_now());
            out.extend_from_slice(&rtc.to_save_bytes());
        }
        out
    }

    /// Advance mapper-side timers (MBC3 RTC) by CPU T-cycles.
    pub fn tick(&mut self, t_cycles: u32) {
        self.mapper.tick(t_cycles);
    }

    /// Return the length of the ROM data.
    pub fn len(&self) -> usize {
        self.rom.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rom.is_empty()
    }

    pub fn rom_bytes(&self) -> &[u8] {
        &self.rom
    }

    pub fn mark_save_dirty(&mut self) {
        if self.has_battery_backed_ram() {
            self.ram_dirty = true;
        }
        if let Some(rtc) = self.mapper.mbc3_rtc_mut() {
            rtc.mark_dirty();
        }
    }

    /// Reset mapper banking to power-on; preserve external SRAM and RTC clock contents.
    pub fn power_on_reset_mapper(&mut self) {
        self.mapper.power_on_reset();
    }

    pub(crate) fn snapshot_mapper(&self) -> crate::snapshot::MapperStateV1 {
        self.mapper.snapshot()
    }

    pub(crate) fn apply_mapper(&mut self, state: &crate::snapshot::MapperStateV1) {
        self.mapper.apply(state);
    }

    pub(crate) fn snapshot_ram(&self) -> Vec<u8> {
        self.ram.clone()
    }

    pub(crate) fn apply_ram(&mut self, ram: &[u8]) {
        let n = ram.len().min(self.ram.len());
        self.ram[..n].copy_from_slice(&ram[..n]);
        if n < self.ram.len() {
            self.ram[n..].fill(0);
        }
    }

    /// Boot ROM–style header validity (logo + header checksum).
    pub fn header_ok(&self) -> bool {
        self.header.is_valid()
    }

    pub fn read8(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x3FFF => {
                let bank = self.fixed_rom_bank() as usize;
                let offset = bank * ROM_BANK_SIZE + addr as usize;
                self.rom_byte(offset)
            }
            0x4000..=0x7FFF => {
                let bank = self.switchable_rom_bank() as usize;
                let offset = (addr as usize - 0x4000) + bank * ROM_BANK_SIZE;
                self.rom_byte(offset)
            }
            0xA000..=0xBFFF => self.read_external_ram(addr),
            _ => 0xFF,
        }
    }

    pub fn write8(&mut self, addr: u16, value: u8) {
        match addr {
            0x0000..=0x7FFF => self.mapper.write_reg(addr, value),
            0xA000..=0xBFFF => self.write_external_ram(addr, value),
            _ => {}
        }
    }

    pub fn switchable_rom_bank(&self) -> u16 {
        let raw = self.mapper.switchable_rom_bank();
        let banks = self
            .header
            .rom_size
            .map(|r| r.bank_count())
            .unwrap_or(2)
            .max(1);
        // Mask into available banks (Pokémon Red: 64 banks, power of two).
        raw % banks
    }

    fn fixed_rom_bank(&self) -> u16 {
        let raw = self.mapper.fixed_rom_bank();
        let banks = self
            .header
            .rom_size
            .map(|r| r.bank_count())
            .unwrap_or(2)
            .max(1);
        raw % banks
    }

    /// File offset for a CPU address in the current ROM mapping, if in `$0000–$7FFF`.
    pub fn physical_rom_offset(&self, addr: u16) -> Option<usize> {
        match addr {
            0x0000..=0x3FFF => {
                let bank = self.fixed_rom_bank() as usize;
                Some(bank * ROM_BANK_SIZE + addr as usize)
            }
            0x4000..=0x7FFF => {
                let bank = self.switchable_rom_bank() as usize;
                Some(bank * ROM_BANK_SIZE + (addr as usize - 0x4000))
            }
            _ => None,
        }
    }

    fn rom_byte(&self, offset: usize) -> u8 {
        self.rom.get(offset).copied().unwrap_or(0xFF)
    }

    fn read_external_ram(&self, addr: u16) -> u8 {
        if let Some(v) = self.mapper.mbc3_read_rtc() {
            return v;
        }
        if let Mapper::Mbc2(m) = &self.mapper {
            return m.read_ram(addr, &self.ram);
        }
        let Some(bank) = self.mapper.active_ram_bank() else {
            return 0xFF;
        };
        let banks = self.header.ram_size.map(|r| r.bank_count()).unwrap_or(0);
        if banks == 0 {
            return 0xFF;
        }
        let bank = (bank % banks) as usize;
        let offset = bank * RAM_BANK_SIZE + (addr as usize - 0xA000);
        self.ram.get(offset).copied().unwrap_or(0xFF)
    }

    fn write_external_ram(&mut self, addr: u16, value: u8) {
        if self.mapper.mbc3_write_rtc(value) {
            return;
        }
        if let Mapper::Mbc2(m) = &self.mapper {
            if m.write_ram(addr, value, &mut self.ram)
                && self.header.cart_type.is_some_and(CartType::has_battery)
            {
                self.ram_dirty = true;
            }
            return;
        }
        let Some(bank) = self.mapper.active_ram_bank() else {
            return;
        };
        let banks = self.header.ram_size.map(|r| r.bank_count()).unwrap_or(0);
        if banks == 0 {
            return;
        }
        let bank = (bank % banks) as usize;
        let offset = bank * RAM_BANK_SIZE + (addr as usize - 0xA000);
        if let Some(slot) = self.ram.get_mut(offset) {
            *slot = value;
            if self.header.cart_type.is_some_and(CartType::has_battery) {
                self.ram_dirty = true;
            }
        }
    }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// External / built-in save RAM length from the header (MBC2 is always 512 B).
fn ram_size_bytes(header: &Header) -> usize {
    if header.cart_type.is_some_and(CartType::is_mbc2) {
        return MBC2_RAM_SIZE;
    }
    header.ram_size.map(|r| r.size_bytes()).unwrap_or(0)
}

#[cfg(test)]
mod tests;
