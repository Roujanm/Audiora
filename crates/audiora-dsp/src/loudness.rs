//! Programme loudness per ITU-R BS.1770-4 / EBU R128: K-weighting, momentary (400 ms),
//! short-term (3 s) and gated integrated loudness. Stereo only (L and R weight 1.0).
//!
//! Allocates once at construction; `process` is allocation-free, but it is meant for an
//! analysis thread rather than the audio callback.

use std::f64::consts::PI;

/// Reported when nothing measurable has been heard yet.
pub const LUFS_FLOOR: f32 = -120.0;

const SUB_BLOCK_MS: f64 = 100.0;
const MOMENTARY_BLOCKS: usize = 4; // 400 ms
const SHORT_TERM_BLOCKS: usize = 30; // 3 s
const ABS_GATE: f64 = -70.0;
const REL_GATE: f64 = -10.0;
const HIST_MIN: f64 = -70.0;
const HIST_STEP: f64 = 0.1;
const HIST_BINS: usize = 800; // -70 .. +10 LUFS

#[derive(Clone, Copy, Debug)]
struct Biquad {
    b: [f64; 3],
    a: [f64; 2], // a1, a2 (a0 = 1)
    z: [f64; 2],
}

impl Biquad {
    fn process(&mut self, x: f64) -> f64 {
        let y = self.b[0] * x + self.z[0];
        self.z[0] = self.b[1] * x - self.a[0] * y + self.z[1];
        self.z[1] = self.b[2] * x - self.a[1] * y;
        y
    }
}

/// Stage 1: the head-related high shelf. Stage 2: the RLB high-pass. Derived from the
/// analog prototypes so any sample rate works; at 48 kHz they equal the coefficients
/// printed in BS.1770 (checked by a test).
fn k_weighting(fs: f64) -> [Biquad; 2] {
    let (f0, g, q) = (1681.974450955533, 3.999843853973347, 0.7071752369554196);
    let k = (PI * f0 / fs).tan();
    let vh = 10f64.powf(g / 20.0);
    let vb = vh.powf(0.4996667741545416);
    let a0 = 1.0 + k / q + k * k;
    let shelf = Biquad {
        b: [(vh + vb * k / q + k * k) / a0, 2.0 * (k * k - vh) / a0, (vh - vb * k / q + k * k) / a0],
        a: [2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0],
        z: [0.0; 2],
    };

    let (f0, q) = (38.13547087602444, 0.5003270373238773);
    let k = (PI * f0 / fs).tan();
    let a0 = 1.0 + k / q + k * k;
    let rlb = Biquad {
        b: [1.0, -2.0, 1.0],
        a: [2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0],
        z: [0.0; 2],
    };
    [shelf, rlb]
}

fn lufs(energy: f64) -> f64 {
    if energy <= 0.0 {
        f64::from(LUFS_FLOOR)
    } else {
        (-0.691 + 10.0 * energy.log10()).max(f64::from(LUFS_FLOOR))
    }
}

pub struct LoudnessMeter {
    filters: [[Biquad; 2]; 2], // [channel][stage]
    sub_len: usize,
    sub_pos: usize,
    sub_sum: f64,
    recent: [f64; SHORT_TERM_BLOCKS],
    written: usize,
    momentary: f64,
    short_term: f64,
    hist_energy: Vec<f64>,
    hist_count: Vec<u64>,
}

impl LoudnessMeter {
    pub fn new(sample_rate: f32) -> Self {
        let fs = f64::from(sample_rate);
        let k = k_weighting(fs);
        Self {
            filters: [k, k],
            sub_len: (fs * SUB_BLOCK_MS / 1000.0).round() as usize,
            sub_pos: 0,
            sub_sum: 0.0,
            recent: [0.0; SHORT_TERM_BLOCKS],
            written: 0,
            momentary: f64::from(LUFS_FLOOR),
            short_term: f64::from(LUFS_FLOOR),
            hist_energy: vec![0.0; HIST_BINS],
            hist_count: vec![0; HIST_BINS],
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new((self.sub_len as f64 * 1000.0 / SUB_BLOCK_MS) as f32);
    }

    /// `stereo` is interleaved L, R.
    pub fn process(&mut self, stereo: &[f32]) {
        for frame in stereo.chunks_exact(2) {
            let mut e = 0.0;
            for (ch, &x) in frame.iter().enumerate() {
                let y = self.filters[ch].iter_mut().fold(f64::from(x), |v, f| f.process(v));
                e += y * y;
            }
            self.sub_sum += e;
            self.sub_pos += 1;
            if self.sub_pos == self.sub_len {
                self.finish_sub_block();
            }
        }
    }

    fn finish_sub_block(&mut self) {
        let energy = self.sub_sum / self.sub_len as f64;
        self.sub_sum = 0.0;
        self.sub_pos = 0;
        self.recent[self.written % SHORT_TERM_BLOCKS] = energy;
        self.written += 1;

        let mean_last = |n: usize, w: usize, r: &[f64; SHORT_TERM_BLOCKS]| {
            (0..n).map(|i| r[(w - 1 - i) % SHORT_TERM_BLOCKS]).sum::<f64>() / n as f64
        };

        if self.written >= MOMENTARY_BLOCKS {
            let e400 = mean_last(MOMENTARY_BLOCKS, self.written, &self.recent);
            self.momentary = lufs(e400);
            // gating histogram, one 400 ms block every 100 ms (75 % overlap)
            if self.momentary > ABS_GATE {
                let bin = (((self.momentary - HIST_MIN) / HIST_STEP) as usize).min(HIST_BINS - 1);
                self.hist_energy[bin] += e400;
                self.hist_count[bin] += 1;
            }
        }
        if self.written >= SHORT_TERM_BLOCKS {
            self.short_term = lufs(mean_last(SHORT_TERM_BLOCKS, self.written, &self.recent));
        }
    }

    /// Loudness of the last 400 ms.
    pub fn momentary(&self) -> f32 {
        self.momentary as f32
    }

    /// Loudness of the last 3 s (floor until 3 s have been measured).
    pub fn short_term(&self) -> f32 {
        self.short_term as f32
    }

    /// Gated programme loudness since the last reset.
    pub fn integrated(&self) -> f32 {
        let (e, n) = self.hist_energy.iter().zip(&self.hist_count).fold((0.0, 0u64), |(e, n), (be, bn)| (e + be, n + bn));
        if n == 0 {
            return LUFS_FLOOR;
        }
        let relative = lufs(e / n as f64) + REL_GATE;
        let (mut e2, mut n2) = (0.0, 0u64);
        for (bin, (be, bn)) in self.hist_energy.iter().zip(&self.hist_count).enumerate() {
            if HIST_MIN + bin as f64 * HIST_STEP >= relative {
                e2 += be;
                n2 += bn;
            }
        }
        if n2 == 0 { LUFS_FLOOR } else { lufs(e2 / n2 as f64) as f32 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FS: f32 = 48_000.0;

    fn sine_stereo(seconds: f32, dbfs: f32, hz: f32, left: bool, right: bool) -> Vec<f32> {
        let a = 10f32.powf(dbfs / 20.0);
        (0..(seconds * FS) as usize)
            .flat_map(|n| {
                let s = a * (2.0 * std::f32::consts::PI * hz * n as f32 / FS).sin();
                [if left { s } else { 0.0 }, if right { s } else { 0.0 }]
            })
            .collect()
    }

    #[test]
    fn k_weighting_matches_the_coefficients_printed_in_bs1770_at_48k() {
        let [shelf, rlb] = k_weighting(48_000.0);
        let near = |a: f64, b: f64| (a - b).abs() < 1e-6;
        assert!(near(shelf.b[0], 1.53512485958697) && near(shelf.b[1], -2.69169618940638) && near(shelf.b[2], 1.19839281085285));
        assert!(near(shelf.a[0], -1.69065929318241) && near(shelf.a[1], 0.73248077421585));
        assert!(near(rlb.a[0], -1.99004745483398) && near(rlb.a[1], 0.99007225036621));
    }

    #[test]
    fn ebu_tech_3341_reference_tone_reads_minus_23_lufs() {
        // stereo 1 kHz sine at -23 dBFS in both channels is -23.0 LUFS
        let mut m = LoudnessMeter::new(FS);
        m.process(&sine_stereo(10.0, -23.0, 1000.0, true, true));
        assert!((m.momentary() + 23.0).abs() < 0.1, "momentary {}", m.momentary());
        assert!((m.short_term() + 23.0).abs() < 0.1, "short-term {}", m.short_term());
        assert!((m.integrated() + 23.0).abs() < 0.1, "integrated {}", m.integrated());
    }

    #[test]
    fn one_silent_channel_is_three_db_quieter() {
        let mut m = LoudnessMeter::new(FS);
        m.process(&sine_stereo(5.0, -23.0, 1000.0, true, false));
        assert!((m.integrated() + 26.0).abs() < 0.15, "integrated {}", m.integrated());
    }

    #[test]
    fn silence_stays_at_the_floor() {
        let mut m = LoudnessMeter::new(FS);
        m.process(&vec![0.0; 2 * 48_000 * 5]);
        assert_eq!(m.momentary(), LUFS_FLOOR);
        assert_eq!(m.integrated(), LUFS_FLOOR);
    }

    #[test]
    fn relative_gate_ignores_a_much_quieter_passage() {
        let mut m = LoudnessMeter::new(FS);
        m.process(&sine_stereo(20.0, -23.0, 1000.0, true, true));
        m.process(&sine_stereo(20.0, -50.0, 1000.0, true, true));
        assert!(m.short_term() < -45.0, "the quiet part is audible to the short-term meter");
        assert!((m.integrated() + 23.0).abs() < 0.15, "gated integrated {}", m.integrated());
    }

    #[test]
    fn louder_by_ten_db_reads_ten_lu_higher_and_reset_clears() {
        let mut a = LoudnessMeter::new(FS);
        let mut b = LoudnessMeter::new(FS);
        a.process(&sine_stereo(6.0, -30.0, 440.0, true, true));
        b.process(&sine_stereo(6.0, -20.0, 440.0, true, true));
        assert!((b.integrated() - a.integrated() - 10.0).abs() < 0.05);
        b.reset();
        assert_eq!(b.integrated(), LUFS_FLOOR);
        assert_eq!(b.momentary(), LUFS_FLOOR);
    }

    #[test]
    fn works_at_other_sample_rates() {
        let mut m = LoudnessMeter::new(44_100.0);
        let a = 10f32.powf(-23.0 / 20.0);
        let x: Vec<f32> = (0..44_100 * 6)
            .flat_map(|n| {
                let s = a * (2.0 * std::f32::consts::PI * 1000.0 * n as f32 / 44_100.0).sin();
                [s, s]
            })
            .collect();
        m.process(&x);
        assert!((m.integrated() + 23.0).abs() < 0.15, "integrated {}", m.integrated());
    }
}
