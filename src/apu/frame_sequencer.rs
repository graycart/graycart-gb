//! 512 Hz frame sequencer (length / sweep / envelope clocks).

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct FrameEvents {
    pub length: bool,
    pub sweep: bool,
    pub envelope: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct FrameSequencer {
    /// Counts T-cycles toward the next 512 Hz step (8192 T).
    timer: u32,
    step: u8,
}

impl FrameSequencer {
    pub(crate) fn new() -> Self {
        Self { timer: 0, step: 0 }
    }

    pub(crate) fn reset(&mut self) {
        self.timer = 0;
        self.step = 0;
    }

    pub(crate) fn step_index(&self) -> u8 {
        self.step
    }

    pub(crate) fn set_step_index(&mut self, step: u8) {
        self.step = step & 7;
    }

    /// True if the *next* frame-sequencer step will clock length (for obscure
    /// NRx4 trigger timing). Approximate for the first slice.
    pub(crate) fn next_frame_will_length(&self) -> bool {
        let next = (self.step + 1) & 7;
        matches!(next, 0 | 2 | 4 | 6)
    }

    pub(crate) fn tick(&mut self, t_cycles: u32, mut on_step: impl FnMut(FrameEvents)) {
        self.timer += t_cycles;
        while self.timer >= 8192 {
            self.timer -= 8192;
            let ev = match self.step {
                0 | 4 => FrameEvents {
                    length: true,
                    sweep: false,
                    envelope: false,
                },
                2 | 6 => FrameEvents {
                    length: true,
                    sweep: true,
                    envelope: false,
                },
                7 => FrameEvents {
                    length: false,
                    sweep: false,
                    envelope: true,
                },
                _ => FrameEvents::default(),
            };
            on_step(ev);
            self.step = (self.step + 1) & 7;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eight_steps_per_frame_at_512hz() {
        let mut seq = FrameSequencer::new();
        let mut lengths = 0;
        let mut envelopes = 0;
        // 8 steps × 8192 = 65536 T = 1/64 s → one full sequencer frame
        seq.tick(8192 * 8, |ev| {
            if ev.length {
                lengths += 1;
            }
            if ev.envelope {
                envelopes += 1;
            }
        });
        assert_eq!(lengths, 4);
        assert_eq!(envelopes, 1);
    }
}
