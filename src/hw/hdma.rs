//! CGB LCD VRAM DMA (`$FF51–$FF55`). Not OAM DMA (`$FF46`).
//!
//! [Pan Docs](https://gbdev.io/pandocs/CGB_Registers.html#lcd-vram-dma-transfers)

mod mmio;
mod timing;
mod transfer;

pub use mmio::{
    HDMA1, HDMA2, HDMA3, HDMA4, HDMA5, Hdma5Write, mask_dest, mask_source, parse_hdma5_write,
    read_hdma5, source_is_vram,
};
pub use timing::{BLOCK_LEN, block_cpu_t, block_fixed_t, hblank_may_transfer};
pub use transfer::{Block, StartOutcome, VramDma, VramDmaMode};

#[cfg(test)]
mod tests;
