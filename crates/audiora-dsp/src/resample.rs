//! Streaming linear-interpolation resampler for interleaved stereo.
//! Placeholder for the drift-tracking sinc SRC planned in the architecture;
//! good enough to bridge devices running at different fixed rates.

pub struct LinearResampler {
    step: f64,
    pos: f64,
    prev: [f32; 2],
}

impl LinearResampler {
    pub fn new(in_rate: u32, out_rate: u32) -> Self {
        Self { step: in_rate as f64 / out_rate as f64, pos: 0.0, prev: [0.0; 2] }
    }

    /// Upper bound on output frames for `in_frames` of input.
    pub fn max_output_frames(&self, in_frames: usize) -> usize {
        ((in_frames as f64 + 1.0) / self.step).ceil() as usize + 2
    }

    /// Returns the number of frames written to `out` (interleaved stereo).
    pub fn process(&mut self, input: &[f32], out: &mut [f32]) -> usize {
        let frames = input.len() / 2;
        if frames == 0 {
            return 0;
        }
        let mut n = 0;
        while self.pos < (frames - 1) as f64 && (n + 1) * 2 <= out.len() {
            let idx = self.pos.floor() as isize;
            let frac = (self.pos - idx as f64) as f32;
            for ch in 0..2 {
                let a = if idx < 0 { self.prev[ch] } else { input[idx as usize * 2 + ch] };
                let b = input[(idx + 1) as usize * 2 + ch];
                out[n * 2 + ch] = a + (b - a) * frac;
            }
            n += 1;
            self.pos += self.step;
        }
        self.pos -= frames as f64;
        self.prev = [input[(frames - 1) * 2], input[(frames - 1) * 2 + 1]];
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dc_is_preserved_and_length_scales() {
        let mut r = LinearResampler::new(16_000, 48_000);
        let input = vec![0.5f32; 160 * 2];
        let mut out = vec![0f32; r.max_output_frames(160) * 2];
        let mut total = 0;
        for block in 0..10 {
            let n = r.process(&input, &mut out);
            let skip = if block == 0 { 4 } else { 0 }; // first frames blend with the zero history
            assert!(out[skip * 2..n * 2].iter().all(|&s| (s - 0.5).abs() < 1e-6));
            total += n;
        }
        assert!((total as i64 - 4800).abs() <= 3, "got {total}");
    }

    #[test]
    fn identity_rate_keeps_samples() {
        let mut r = LinearResampler::new(48_000, 48_000);
        let input: Vec<f32> = (0..64).map(|v| v as f32).collect();
        let mut out = vec![0f32; 200];
        let n = r.process(&input, &mut out);
        assert!(n >= 31);
        for i in 0..n {
            assert_eq!(out[i * 2], input[i * 2]);
        }
    }
}
