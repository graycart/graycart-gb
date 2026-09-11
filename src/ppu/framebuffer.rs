//! DMG framebuffer (shades) plus optional NativeCgb RGB555 for host present.

/// Visible LCD width in pixels.
pub const SCREEN_WIDTH: usize = 160;
/// Visible LCD height in pixels.
pub const SCREEN_HEIGHT: usize = 144;

/// One of four DMG gray shades (0 = lightest / white, 3 = darkest / black).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum Shade {
    #[default]
    Lightest = 0,
    Light = 1,
    Dark = 2,
    Darkest = 3,
}

impl Shade {
    pub fn from_index(index: u8) -> Self {
        match index & 0b11 {
            0 => Self::Lightest,
            1 => Self::Light,
            2 => Self::Dark,
            _ => Self::Darkest,
        }
    }

    pub fn index(self) -> u8 {
        self as u8
    }
}

/// Map a tile/OBJ color ID (0–3) through BGP (`$FF47`) to a display shade.
///
/// BGP packs four 2-bit shade numbers: bits 1–0 = color 0, 3–2 = color 1, etc.
pub fn shade_from_bgp(color_id: u8, bgp: u8) -> Shade {
    let shift = (color_id & 0b11) * 2;
    Shade::from_index(bgp >> shift)
}

/// 160×144 LCD buffer: shades always; RGB555 when NativeCgb presents color.
#[derive(Debug, Clone)]
pub struct Framebuffer {
    pixels: [Shade; SCREEN_WIDTH * SCREEN_HEIGHT],
    rgb555: [u16; SCREEN_WIDTH * SCREEN_HEIGHT],
    presents_cgb_color: bool,
}

impl Default for Framebuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl Framebuffer {
    pub fn new() -> Self {
        Self {
            pixels: [Shade::Lightest; SCREEN_WIDTH * SCREEN_HEIGHT],
            rgb555: [0x7FFF; SCREEN_WIDTH * SCREEN_HEIGHT],
            presents_cgb_color: false,
        }
    }

    pub fn clear(&mut self, shade: Shade) {
        self.pixels.fill(shade);
        let rgb = if shade == Shade::Lightest { 0x7FFF } else { 0 };
        self.rgb555.fill(rgb);
    }

    pub fn set_pixel(&mut self, x: usize, y: usize, shade: Shade) {
        if let Some(slot) = self.pixels.get_mut(y * SCREEN_WIDTH + x) {
            *slot = shade;
        }
    }

    /// NativeCgb mix: store shade (tests/compat) and RGB555 (host present).
    pub fn set_cgb_pixel(&mut self, x: usize, y: usize, shade: Shade, rgb555: u16) {
        let i = y * SCREEN_WIDTH + x;
        if let Some(slot) = self.pixels.get_mut(i) {
            *slot = shade;
            self.rgb555[i] = rgb555;
        }
    }

    pub fn set_presents_cgb_color(&mut self, enabled: bool) {
        self.presents_cgb_color = enabled;
    }

    pub fn presents_cgb_color(&self) -> bool {
        self.presents_cgb_color
    }

    pub fn pixel(&self, x: usize, y: usize) -> Shade {
        self.pixels
            .get(y * SCREEN_WIDTH + x)
            .copied()
            .unwrap_or(Shade::Lightest)
    }

    pub fn rgb555_pixel(&self, x: usize, y: usize) -> u16 {
        self.rgb555.get(y * SCREEN_WIDTH + x).copied().unwrap_or(0)
    }

    pub fn pixels(&self) -> &[Shade] {
        &self.pixels
    }

    pub fn rgb555_pixels(&self) -> &[u16] {
        &self.rgb555
    }
}

#[cfg(test)]
mod tests;
