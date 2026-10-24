//! Noise suppression using a Rust port of RNNoise (`nnnoiseless`).
//! Works on 48 kHz audio in 480-frame blocks, so it adds exactly 10 ms of
//! latency. Other sample rates must bypass it.

use nnnoiseless::DenoiseState;

pub const DENOISE_FRAME: usize = nnnoiseless::FRAME_SIZE;
pub const DENOISE_SAMPLE_RATE: u32 = 48_000;

const SCALE: f32 = 32768.0; // RNNoise expects 16-bit-range floats

struct Channel {
    state: Box<DenoiseState<'static>>,
    inp: [f32; DENOISE_FRAME],
    out: [f32; DENOISE_FRAME],
}

impl Channel {
    fn new() -> Self {
        Self { state: DenoiseState::new(), inp: [0.0; DENOISE_FRAME], out: [0.0; DENOISE_FRAME] }
    }
}

pub struct Denoiser {
    ch: [Channel; 2],
    pos: usize,
}

impl Default for Denoiser {
    fn default() -> Self {
        Self::new()
    }
}

impl Denoiser {
    pub fn new() -> Self {
        Self { ch: [Channel::new(), Channel::new()], pos: 0 }
    }

    pub const fn latency_frames() -> usize {
        DENOISE_FRAME
    }

    /// In-place on interleaved stereo.
    pub fn process(&mut self, buf: &mut [f32]) {
        for f in buf.chunks_exact_mut(2) {
            for (c, s) in self.ch.iter_mut().zip(f.iter_mut()) {
                let delayed = c.out[self.pos];
                c.inp[self.pos] = *s * SCALE;
                *s = delayed / SCALE;
            }
            self.pos += 1;
            if self.pos == DENOISE_FRAME {
                self.pos = 0;
                for c in self.ch.iter_mut() {
                    let Channel { state, inp, out } = c;
                    state.process_frame(out, inp);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
    }

    #[test]
    fn steady_noise_is_strongly_attenuated() {
        let mut d = Denoiser::new();
        // deterministic white noise at about -30 dBFS
        let mut seed = 12345u32;
        let mut noise = || {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            ((seed >> 8) as f32 / (1u32 << 24) as f32 - 0.5) * 0.06
        };
        let mut x: Vec<f32> = (0..48_000 * 2).map(|_| noise()).collect();
        let before = rms(&x[48_000..]);
        for chunk in x.chunks_mut(300) {
            d.process(chunk);
        }
        let after = rms(&x[48_000..]);
        assert!(after < before * 0.5, "before {before} after {after}");
    }

    #[test]
    fn silence_stays_silent_and_latency_is_one_frame() {
        let mut d = Denoiser::new();
        let mut x = vec![0.0f32; 4800 * 2];
        d.process(&mut x);
        assert!(x.iter().all(|&v| v.abs() < 1e-6));
        assert_eq!(Denoiser::latency_frames(), 480);
    }
}
