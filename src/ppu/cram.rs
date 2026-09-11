//! CGB palette RAM ports (`BGPI`/`BGPD`, `OBPI`/`OBPD`).
//!
//! Two independent 64-byte CRAM banks. No color rendering lives here.

pub const BGPI: u16 = 0xFF68;
pub const BGPD: u16 = 0xFF69;
pub const OBPI: u16 = 0xFF6A;
pub const OBPD: u16 = 0xFF6B;

const CRAM_SIZE: usize = 64;
const ADDR_MASK: u8 = 0x3F;
const AUTO_INC: u8 = 0x80;
const UNUSED_BIT6: u8 = 0x40;

#[derive(Debug, Clone)]
struct CramPort {
    ram: [u8; CRAM_SIZE],
    /// Bit 7 = auto-increment; bits 5–0 = byte address. Bit 6 is not stored.
    index: u8,
}

impl CramPort {
    fn new() -> Self {
        Self {
            ram: [0; CRAM_SIZE],
            index: 0,
        }
    }

    fn read_index(&self) -> u8 {
        self.index | UNUSED_BIT6
    }

    fn write_index(&mut self, value: u8) {
        self.index = value & (AUTO_INC | ADDR_MASK);
    }

    fn address(&self) -> usize {
        usize::from(self.index & ADDR_MASK)
    }

    fn maybe_increment(&mut self) {
        if self.index & AUTO_INC != 0 {
            let next = (self.index & ADDR_MASK).wrapping_add(1) & ADDR_MASK;
            self.index = AUTO_INC | next;
        }
    }

    fn read_data(&self, accessible: bool) -> u8 {
        if accessible {
            self.ram[self.address()]
        } else {
            0xFF
        }
    }

    fn write_data(&mut self, value: u8, accessible: bool) {
        if accessible {
            let addr = self.address();
            self.ram[addr] = value;
        }
        self.maybe_increment();
    }

    fn rgb555(&self, palette: u8, color: u8) -> u16 {
        let offset = Self::color_offset(palette, color);
        u16::from(self.ram[offset]) | u16::from(self.ram[offset + 1]) << 8
    }

    fn color_offset(palette: u8, color: u8) -> usize {
        usize::from(palette) * 8 + usize::from(color) * 2
    }

    fn write_rgb555(&mut self, palette: u8, color: u8, rgb: u16) {
        let offset = Self::color_offset(palette, color);
        self.ram[offset] = rgb as u8;
        self.ram[offset + 1] = (rgb >> 8) as u8;
    }

    fn fill_all_white(&mut self) {
        for pal in 0..8 {
            for color in 0..4 {
                self.write_rgb555(pal, color, 0x7FFF);
            }
        }
    }

    fn fill_palette_gray_ramp(&mut self, palette: u8) {
        const RAMP: [u16; 4] = [0x7FFF, 0x5294, 0x294A, 0x0000];
        for (color, rgb) in RAMP.into_iter().enumerate() {
            self.write_rgb555(palette, color as u8, rgb);
        }
    }
}

/// Background and object Color RAM, each 64 bytes, with independent index ports.
#[derive(Debug, Clone)]
pub struct Cram {
    bg: CramPort,
    obj: CramPort,
}

impl Default for Cram {
    fn default() -> Self {
        Self::new()
    }
}

impl Cram {
    pub fn new() -> Self {
        Self {
            bg: CramPort::new(),
            obj: CramPort::new(),
        }
    }

    pub fn read_bgpi(&self) -> u8 {
        self.bg.read_index()
    }

    pub fn write_bgpi(&mut self, value: u8) {
        self.bg.write_index(value);
    }

    pub fn read_bgpd(&self, accessible: bool) -> u8 {
        self.bg.read_data(accessible)
    }

    pub fn write_bgpd(&mut self, value: u8, accessible: bool) {
        self.bg.write_data(value, accessible);
    }

    pub fn read_obpi(&self) -> u8 {
        self.obj.read_index()
    }

    pub fn write_obpi(&mut self, value: u8) {
        self.obj.write_index(value);
    }

    pub fn read_obpd(&self, accessible: bool) -> u8 {
        self.obj.read_data(accessible)
    }

    pub fn write_obpd(&mut self, value: u8, accessible: bool) {
        self.obj.write_data(value, accessible);
    }

    /// palette 0–7, color 0–3; little-endian u16 of the two CRAM bytes.
    pub fn rgb555_bg(&self, palette: u8, color: u8) -> u16 {
        self.bg.rgb555(palette, color)
    }

    pub fn rgb555_obj(&self, palette: u8, color: u8) -> u16 {
        self.obj.rgb555(palette, color)
    }

    /// All 8 BG palettes, all 4 colors: RGB555 0x7FFF (white). OBJ untouched.
    pub fn fill_bg_white(&mut self) {
        self.bg.fill_all_white();
    }

    /// Palette ID $00 stand-in: BG pal 0 and OBJ pal 0+1 gray ramp for BGP=$FC style
    /// color0=0x7FFF, color1=0x5294, color2=0x294A, color3=0x0000 (little-endian in CRAM).
    /// Remaining palettes stay `$FF` (CGB power-on / `boot_hwio-C` BGPD).
    pub fn fill_compat_gray_ramp(&mut self) {
        self.bg.ram = [0xFF; CRAM_SIZE];
        self.obj.ram = [0xFF; CRAM_SIZE];
        self.bg.fill_palette_gray_ramp(0);
        self.obj.fill_palette_gray_ramp(0);
        self.obj.fill_palette_gray_ramp(1);
    }
}

#[cfg(test)]
mod tests;
