use super::*;
use crate::ppu::Ppu;

#[test]
fn power_on_lcdc_is_zero_and_lcd_disabled() {
    let regs = Registers::new();
    assert_eq!(regs.lcdc, LCDC_POWER_ON);
    assert_eq!(regs.lcdc, 0);
    assert!(!regs.lcd_enabled());

    let mut ppu = Ppu::new();
    assert_eq!(ppu.lcdc(), LCDC_POWER_ON);
    assert!(!ppu.lcd_enabled());

    ppu.write(LCDC, LCDC_AFTER_BOOT);
    assert!(ppu.lcd_enabled());
    ppu.power_on_reset();
    assert_eq!(ppu.lcdc(), LCDC_POWER_ON);
    assert!(!ppu.lcd_enabled());
}
