//! Host-facing PCM buffer (stereo f32, interleaved as sample pairs).

/// Target host sample rate for the first audio slice.
pub const HOST_SAMPLE_RATE: u32 = 48_000;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StereoSample {
    pub left: f32,
    pub right: f32,
}

impl StereoSample {
    pub const SILENCE: Self = Self {
        left: 0.0,
        right: 0.0,
    };
}

#[derive(Debug, Clone, Default)]
pub struct SampleBuffer {
    samples: Vec<StereoSample>,
}

impl SampleBuffer {
    pub fn new() -> Self {
        Self {
            samples: Vec::with_capacity(4096),
        }
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    pub fn push(&mut self, sample: StereoSample) {
        // Cap to avoid unbounded growth if the host falls behind.
        const MAX: usize = 48_000; // ~1s
        if self.samples.len() >= MAX {
            let drop = MAX / 4;
            self.samples.drain(..drop);
        }
        self.samples.push(sample);
    }

    pub fn drain(&mut self) -> Vec<StereoSample> {
        std::mem::take(&mut self.samples)
    }
}
