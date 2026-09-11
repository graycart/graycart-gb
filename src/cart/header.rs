use super::cart_type::CartType;
use super::destination::Destination;
use super::licensee;
use super::ram_size::RamSize;
use super::rom_size::RomSize;
use super::{NINTENDO_LOGO, OLD_LICENSEE_USE_NEW, SGB_FLAG_ENABLED, UNKNOWN};
use std::fmt;

/// Error while parsing a cartridge header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderError {
    /// ROM shorter than `$0150` bytes.
    TooShort { len: usize },
}

impl fmt::Display for HeaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort { len } => {
                write!(
                    f,
                    "ROM too short for cartridge header ({len} bytes, need at least 0x150)"
                )
            }
        }
    }
}

impl std::error::Error for HeaderError {}

/// CGB support byte at header offset 0x0143 (cartridge capability, not hardware model).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CartridgeCgbSupport {
    /// No CGB features (typical DMG titles); bit 7 clear.
    DmgOnly,
    /// `$80` — CGB enhancements, DMG compatible.
    CgbCompatible,
    /// `$C0` — CGB only.
    CgbOnly,
    /// Other value (still may have bit 7 set on some homebrew).
    Other(u8),
}

impl CartridgeCgbSupport {
    pub fn from_byte(byte: u8) -> Self {
        match byte {
            0x80 => Self::CgbCompatible,
            0xC0 => Self::CgbOnly,
            b if b & 0x80 == 0 => Self::DmgOnly,
            b => Self::Other(b),
        }
    }

    /// True when bit 7 is set (CGB mode / shortened title field).
    pub fn is_cgb(self) -> bool {
        match self {
            Self::CgbCompatible | Self::CgbOnly => true,
            Self::Other(b) => b & 0x80 != 0,
            Self::DmgOnly => false,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::CgbCompatible => "CGB compatible (works on DMG too)",
            Self::CgbOnly => "CGB only",
            Self::DmgOnly => "DMG / non-CGB",
            Self::Other(_) => "unknown CGB flag",
        }
    }
}

/// Compatibility alias: header `$0143` is cartridge CGB capability, not a hardware-model switch.
pub type CgbFlag = CartridgeCgbSupport;

/// Parsed cartridge header (`$0100`–`$014F`).
#[derive(Debug, Clone)]
pub struct Header {
    pub entry_point: [u8; 4],
    pub logo_ok: bool,
    pub title: String,
    pub manufacturer: [u8; 4],
    pub cgb_byte: u8,
    pub cgb: CartridgeCgbSupport,
    pub new_licensee: [u8; 2],
    pub sgb_byte: u8,
    pub sgb: bool,
    pub cart_type_byte: u8,
    pub cart_type: Option<CartType>,
    pub rom_size_byte: u8,
    pub rom_size: Option<RomSize>,
    pub ram_size_byte: u8,
    pub ram_size: Option<RamSize>,
    pub destination_byte: u8,
    pub destination: Option<Destination>,
    pub old_licensee: u8,
    pub mask_rom_version: u8,
    pub header_checksum: u8,
    pub computed_header_checksum: u8,
    pub global_checksum: u16,
    pub computed_global_checksum: u16,
}

impl Header {
    /// Parse the header from a full ROM image.
    pub fn parse(rom: &[u8]) -> Result<Self, HeaderError> {
        if rom.len() < 0x150 {
            return Err(HeaderError::TooShort { len: rom.len() });
        }

        let cgb_byte = rom[0x0143];
        let cgb = CartridgeCgbSupport::from_byte(cgb_byte);
        let title_end = if cgb.is_cgb() { 0x013F } else { 0x0144 };
        let sgb_byte = rom[0x0146];

        let computed_header_checksum = {
            let mut checksum: u8 = 0;
            for &b in &rom[0x0134..=0x014C] {
                checksum = checksum.wrapping_sub(b).wrapping_sub(1);
            }
            checksum
        };

        let computed_global_checksum = {
            let mut sum: u16 = 0;
            for (i, &b) in rom.iter().enumerate() {
                if i == 0x014E || i == 0x014F {
                    continue;
                }
                sum = sum.wrapping_add(u16::from(b));
            }
            sum
        };

        let cart_type_byte = rom[0x0147];
        let rom_size_byte = rom[0x0148];
        let ram_size_byte = rom[0x0149];
        let destination_byte = rom[0x014A];
        let header_checksum = rom[0x014D];
        let global_checksum = u16::from_be_bytes([rom[0x014E], rom[0x014F]]);

        Ok(Self {
            entry_point: rom[0x0100..0x0104].try_into().unwrap(),
            logo_ok: rom[0x0104..0x0134] == NINTENDO_LOGO,
            title: ascii_title(&rom[0x0134..title_end]),
            manufacturer: rom[0x013F..0x0143].try_into().unwrap(),
            cgb_byte,
            cgb,
            new_licensee: rom[0x0144..0x0146].try_into().unwrap(),
            sgb_byte,
            sgb: sgb_byte == SGB_FLAG_ENABLED,
            cart_type_byte,
            cart_type: CartType::from_byte(cart_type_byte),
            rom_size_byte,
            rom_size: RomSize::from_byte(rom_size_byte),
            ram_size_byte,
            ram_size: RamSize::from_byte(ram_size_byte),
            destination_byte,
            destination: Destination::from_byte(destination_byte),
            old_licensee: rom[0x014B],
            mask_rom_version: rom[0x014C],
            header_checksum,
            computed_header_checksum,
            global_checksum,
            computed_global_checksum,
        })
    }

    pub fn header_checksum_ok(&self) -> bool {
        self.header_checksum == self.computed_header_checksum
    }

    pub fn global_checksum_ok(&self) -> bool {
        self.global_checksum == self.computed_global_checksum
    }

    /// Boot ROM–style validity: Nintendo logo + header checksum.
    pub fn is_valid(&self) -> bool {
        self.logo_ok && self.header_checksum_ok()
    }

    pub fn uses_new_licensee(&self) -> bool {
        self.old_licensee == OLD_LICENSEE_USE_NEW
    }

    pub fn publisher(&self) -> Option<&'static str> {
        if self.uses_new_licensee() {
            licensee::new_publisher_name(&self.new_licensee)
        } else {
            licensee::old_publisher_name(self.old_licensee)
        }
    }

    pub fn cart_type_name(&self) -> &'static str {
        self.cart_type.map(CartType::name).unwrap_or(UNKNOWN)
    }

    pub fn rom_size_name(&self) -> &'static str {
        self.rom_size.map(RomSize::name).unwrap_or(UNKNOWN)
    }

    pub fn ram_size_name(&self) -> &'static str {
        self.ram_size.map(RamSize::name).unwrap_or(UNKNOWN)
    }

    pub fn destination_name(&self) -> &'static str {
        self.destination.map(Destination::name).unwrap_or(UNKNOWN)
    }

    pub fn publisher_name(&self) -> &'static str {
        self.publisher().unwrap_or(UNKNOWN)
    }

    pub fn new_licensee_publisher(&self) -> Option<&'static str> {
        licensee::new_publisher_name(&self.new_licensee)
    }

    pub fn old_licensee_publisher(&self) -> Option<&'static str> {
        licensee::old_publisher_name(self.old_licensee)
    }
}

impl fmt::Display for Header {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "--- cartridge header ---")?;
        writeln!(f, "entry point:      {}", hex_bytes(&self.entry_point))?;
        writeln!(
            f,
            "nintendo logo:    {}",
            if self.logo_ok { "ok" } else { "MISMATCH" }
        )?;
        writeln!(f, "title:            {}", self.title)?;
        writeln!(
            f,
            "manufacturer:     {} ({})",
            ascii_graphic(&self.manufacturer),
            hex_bytes(&self.manufacturer)
        )?;
        writeln!(
            f,
            "cgb flag:         ${:02X} ({})",
            self.cgb_byte,
            self.cgb.name()
        )?;
        writeln!(
            f,
            "new licensee:     {} ({})",
            ascii_graphic(&self.new_licensee),
            self.new_licensee_publisher().unwrap_or(UNKNOWN)
        )?;
        writeln!(
            f,
            "sgb flag:         ${:02X} ({})",
            self.sgb_byte,
            if self.sgb {
                "SGB functions supported"
            } else {
                "No SGB functions (ignored by SGB)"
            }
        )?;
        writeln!(
            f,
            "cart type:        ${:02X} ({})",
            self.cart_type_byte,
            self.cart_type_name()
        )?;

        match self.rom_size {
            Some(rom) => writeln!(
                f,
                "rom size:         ${:02X} ({} / {} banks / {} bytes)",
                self.rom_size_byte,
                rom.name(),
                rom.bank_count(),
                rom.size_bytes()
            )?,
            None => writeln!(
                f,
                "rom size:         ${:02X} ({})",
                self.rom_size_byte,
                self.rom_size_name()
            )?,
        }

        match self.ram_size {
            Some(ram) => writeln!(
                f,
                "ram size:         ${:02X} ({} / {} banks / {} bytes)",
                self.ram_size_byte,
                ram.name(),
                ram.bank_count(),
                ram.size_bytes()
            )?,
            None => writeln!(
                f,
                "ram size:         ${:02X} ({})",
                self.ram_size_byte,
                self.ram_size_name()
            )?,
        }

        writeln!(
            f,
            "destination:      ${:02X} ({})",
            self.destination_byte,
            self.destination_name()
        )?;
        writeln!(
            f,
            "old licensee:     ${:02X} ({})",
            self.old_licensee,
            self.old_licensee_publisher().unwrap_or(UNKNOWN)
        )?;
        writeln!(
            f,
            "publisher:        {} ({})",
            self.publisher_name(),
            if self.uses_new_licensee() {
                "new licensee"
            } else {
                "old licensee"
            }
        )?;
        writeln!(f, "mask rom version: ${:02X}", self.mask_rom_version)?;
        writeln!(
            f,
            "header checksum:  ${:02X} (computed ${:02X}) {}",
            self.header_checksum,
            self.computed_header_checksum,
            if self.header_checksum_ok() {
                "ok"
            } else {
                "FAIL"
            }
        )?;
        write!(
            f,
            "global checksum:  ${:04X} (computed ${:04X}) {}",
            self.global_checksum,
            self.computed_global_checksum,
            if self.global_checksum_ok() {
                "ok"
            } else {
                "FAIL"
            }
        )?;
        Ok(())
    }
}

fn ascii_title(raw: &[u8]) -> String {
    let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
    String::from_utf8_lossy(&raw[..end]).trim().to_string()
}

fn ascii_graphic(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&b| {
            let c = b as char;
            if c.is_ascii_graphic() { c } else { '.' }
        })
        .collect()
}

fn hex_bytes(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests;
