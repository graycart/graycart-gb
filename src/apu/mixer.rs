//! NR50 / NR51 mixer → stereo float samples.

#[derive(Debug, Clone)]
pub(crate) struct Mixer {
    nr50: u8,
    nr51: u8,
}

impl Mixer {
    pub(crate) fn new() -> Self {
        Self { nr50: 0, nr51: 0 }
    }

    pub(crate) fn power_off(&mut self) {
        self.nr50 = 0;
        self.nr51 = 0;
    }

    pub(crate) fn read_nr50(&self) -> u8 {
        self.nr50
    }
    pub(crate) fn read_nr51(&self) -> u8 {
        self.nr51
    }
    pub(crate) fn write_nr50(&mut self, value: u8) {
        self.nr50 = value;
    }
    pub(crate) fn write_nr51(&mut self, value: u8) {
        self.nr51 = value;
    }

    /// Mix CH1–CH4 mono into stereo with NR51 routing + NR50 volumes.
    ///
    /// NR51: bit0–3 = CH1–4 → R, bit4–7 = CH1–4 → L.
    pub(crate) fn mix(&self, ch1: f32, ch2: f32, ch3: f32, ch4: f32) -> (f32, f32) {
        let left_vol = (self.nr50 >> 4) & 0x07;
        let right_vol = self.nr50 & 0x07;
        let scale = |v: u8| (f32::from(v) + 1.0) / 8.0;
        let ls = scale(left_vol);
        let rs = scale(right_vol);

        let chans = [ch1, ch2, ch3, ch4];
        let mut l = 0.0;
        let mut r = 0.0;
        for (i, &ch) in chans.iter().enumerate() {
            let bit = 1u8 << i;
            if self.nr51 & (bit << 4) != 0 {
                l += ch * ls;
            }
            if self.nr51 & bit != 0 {
                r += ch * rs;
            }
        }
        (l.clamp(-1.0, 1.0), r.clamp(-1.0, 1.0))
    }
}
