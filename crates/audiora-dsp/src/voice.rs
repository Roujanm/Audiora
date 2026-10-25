//! Voice-specific processors: a steep low cut and a de-esser.
//! Interleaved stereo, no allocation after construction.

use crate::{db_to_lin, lin_to_db, Biquad, BiquadCoeffs, FilterKind};

/// 24 dB/octave Butterworth high-pass: removes rumble, desk thumps and most plosive "pops" without
/// thinning the voice.
pub struct LowCut {
    fs: f32,
    freq: f32,
    stages: [[Biquad; 2]; 2],
}

/// Q of the two biquads that make a 4th-order Butterworth.
const BUTTERWORTH4_Q: [f32; 2] = [0.541_196_1, 1.306_563];

impl LowCut {
    pub fn new(sample_rate: f32) -> Self {
        let mut me = Self { fs: sample_rate, freq: 0.0, stages: [[Biquad::default(); 2]; 2] };
        me.set_freq(80.0);
        me
    }

    fn set_freq(&mut self, freq: f32) {
        self.freq = freq;
        for (stage, q) in self.stages.iter_mut().zip(BUTTERWORTH4_Q) {
            let c = BiquadCoeffs::new(FilterKind::HighPass, self.fs, freq, q, 0.0);
            stage[0].set_coeffs(c);
            stage[1].set_coeffs(c);
        }
    }

    pub fn process(&mut self, buf: &mut [f32], freq: f32) {
        if (freq - self.freq).abs() > 0.5 {
            self.set_freq(freq);
        }
        for f in buf.chunks_exact_mut(2) {
            for stage in self.stages.iter_mut() {
                f[0] = stage[0].process(f[0]);
                f[1] = stage[1].process(f[1]);
            }
        }
    }

    pub fn reset(&mut self) {
        for stage in self.stages.iter_mut() {
            stage[0].reset();
            stage[1].reset();
        }
    }
}

/// Dynamic-EQ de-esser: listens to the sibilance band and dips only that band when "s" and "sh"
/// sounds get harsh, leaving the rest of the voice untouched.
pub struct DeEsser {
    fs: f32,
    /// Two 2nd-order high-passes per channel: only the sibilance region reaches the detector.
    detect: [[Biquad; 2]; 2],
    cut: [Biquad; 2],
    env: f32,
    gr_db: f32,
    applied_db: f32,
}

const DEESS_FREQ: f32 = 6500.0;
const DEESS_MAX_DB: f32 = 12.0;
/// Coefficients are recomputed this often (samples); in between the dip is constant.
const DEESS_STEP: usize = 32;

impl DeEsser {
    pub fn new(sample_rate: f32) -> Self {
        let hp = BiquadCoeffs::new(FilterKind::HighPass, sample_rate, 4500.0, 0.707, 0.0);
        let mut detect = [[Biquad::default(); 2]; 2];
        for stage in detect.iter_mut() {
            stage[0].set_coeffs(hp);
            stage[1].set_coeffs(hp);
        }
        Self { fs: sample_rate, detect, cut: [Biquad::default(); 2], env: 0.0, gr_db: 0.0, applied_db: f32::NAN }
    }

    /// `threshold_db`: sibilance level (dBFS, in the detection band) above which the dip starts.
    /// Returns the current dip in dB (>= 0).
    pub fn process(&mut self, buf: &mut [f32], threshold_db: f32) -> f32 {
        let att = (-1.0 / (0.0008 * self.fs)).exp();
        let rel = (-1.0 / (0.06 * self.fs)).exp();
        for block in buf.chunks_mut(DEESS_STEP * 2) {
            for f in block.chunks_exact(2) {
                let (mut l, mut r) = (f[0], f[1]);
                for stage in self.detect.iter_mut() {
                    l = stage[0].process(l);
                    r = stage[1].process(r);
                }
                let level = l.abs().max(r.abs());
                let c = if level > self.env { att } else { rel };
                self.env = level + (self.env - level) * c;
            }
            let over = lin_to_db(self.env) - threshold_db;
            let target = (over * 0.7).clamp(0.0, DEESS_MAX_DB);
            self.gr_db = target;
            if (self.gr_db - self.applied_db).abs() > 0.05 || self.applied_db.is_nan() {
                let c = if self.gr_db < 0.05 {
                    BiquadCoeffs::IDENTITY
                } else {
                    BiquadCoeffs::new(FilterKind::Peak, self.fs, DEESS_FREQ, 1.0, -self.gr_db)
                };
                self.cut[0].set_coeffs(c);
                self.cut[1].set_coeffs(c);
                self.applied_db = self.gr_db;
            }
            for f in block.chunks_exact_mut(2) {
                f[0] = self.cut[0].process(f[0]);
                f[1] = self.cut[1].process(f[1]);
            }
        }
        self.gr_db
    }

    pub fn reset(&mut self) {
        self.env = 0.0;
        self.gr_db = 0.0;
        self.applied_db = f32::NAN;
        for b in self.detect.iter_mut().flatten().chain(self.cut.iter_mut()) {
            b.reset();
        }
    }
}

/// Linear gain helper used by tests.
#[allow(dead_code)]
fn gain(db: f32) -> f32 {
    db_to_lin(db)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(freq: f32, amp: f32, frames: usize) -> Vec<f32> {
        (0..frames)
            .flat_map(|n| {
                let s = amp * (2.0 * std::f32::consts::PI * freq * n as f32 / 48_000.0).sin();
                [s, s]
            })
            .collect()
    }

    fn peak(x: &[f32]) -> f32 {
        x[x.len() / 2..].iter().fold(0.0, |m, v| m.max(v.abs()))
    }

    #[test]
    fn low_cut_removes_rumble_and_keeps_the_voice() {
        let mut lc = LowCut::new(48_000.0);
        let mut rumble = sine(30.0, 0.5, 48_000);
        lc.process(&mut rumble, 80.0);
        assert!(peak(&rumble) < 0.5 * gain(-20.0), "30 Hz at {}", peak(&rumble));
        let mut lc = LowCut::new(48_000.0);
        let mut voice = sine(300.0, 0.5, 48_000);
        lc.process(&mut voice, 80.0);
        assert!(peak(&voice) > 0.5 * gain(-0.5), "300 Hz at {}", peak(&voice));
    }

    #[test]
    fn de_esser_dips_harsh_sibilance_only() {
        let mut d = DeEsser::new(48_000.0);
        let mut ess = sine(6500.0, 0.5, 24_000);
        let gr = d.process(&mut ess, -30.0);
        assert!(gr > 6.0, "gr {gr}");
        assert!(peak(&ess) < 0.5 * gain(-4.0));

        let mut d = DeEsser::new(48_000.0);
        let mut vowel = sine(500.0, 0.5, 24_000);
        let gr = d.process(&mut vowel, -30.0);
        assert!(gr < 0.5, "gr {gr}");
        assert!(peak(&vowel) > 0.5 * gain(-0.5));
    }
}
