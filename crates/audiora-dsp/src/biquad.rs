//! Biquad filters (RBJ cookbook), transposed direct form II, f64 state.

use std::f64::consts::PI;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterKind {
    LowShelf,
    HighShelf,
    Peak,
    LowPass,
    HighPass,
}

#[derive(Clone, Copy, Debug)]
pub struct BiquadCoeffs {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
}

impl BiquadCoeffs {
    pub const IDENTITY: Self = Self { b0: 1.0, b1: 0.0, b2: 0.0, a1: 0.0, a2: 0.0 };

    /// `gain_db` is ignored for low/high-pass.
    pub fn new(kind: FilterKind, sample_rate: f32, freq: f32, q: f32, gain_db: f32) -> Self {
        let fs = sample_rate as f64;
        let freq = (freq as f64).clamp(10.0, fs * 0.49);
        let q = (q as f64).max(0.05);
        let a = 10f64.powf(gain_db as f64 / 40.0);
        let w0 = 2.0 * PI * freq / fs;
        let (sin, cos) = w0.sin_cos();
        let alpha = sin / (2.0 * q);
        let sqa2alpha = 2.0 * a.sqrt() * alpha;

        let (b0, b1, b2, a0, a1, a2) = match kind {
            FilterKind::Peak => (
                1.0 + alpha * a,
                -2.0 * cos,
                1.0 - alpha * a,
                1.0 + alpha / a,
                -2.0 * cos,
                1.0 - alpha / a,
            ),
            FilterKind::LowShelf => (
                a * ((a + 1.0) - (a - 1.0) * cos + sqa2alpha),
                2.0 * a * ((a - 1.0) - (a + 1.0) * cos),
                a * ((a + 1.0) - (a - 1.0) * cos - sqa2alpha),
                (a + 1.0) + (a - 1.0) * cos + sqa2alpha,
                -2.0 * ((a - 1.0) + (a + 1.0) * cos),
                (a + 1.0) + (a - 1.0) * cos - sqa2alpha,
            ),
            FilterKind::HighShelf => (
                a * ((a + 1.0) + (a - 1.0) * cos + sqa2alpha),
                -2.0 * a * ((a - 1.0) + (a + 1.0) * cos),
                a * ((a + 1.0) + (a - 1.0) * cos - sqa2alpha),
                (a + 1.0) - (a - 1.0) * cos + sqa2alpha,
                2.0 * ((a - 1.0) - (a + 1.0) * cos),
                (a + 1.0) - (a - 1.0) * cos - sqa2alpha,
            ),
            FilterKind::LowPass => (
                (1.0 - cos) / 2.0,
                1.0 - cos,
                (1.0 - cos) / 2.0,
                1.0 + alpha,
                -2.0 * cos,
                1.0 - alpha,
            ),
            FilterKind::HighPass => (
                (1.0 + cos) / 2.0,
                -(1.0 + cos),
                (1.0 + cos) / 2.0,
                1.0 + alpha,
                -2.0 * cos,
                1.0 - alpha,
            ),
        };

        Self { b0: b0 / a0, b1: b1 / a0, b2: b2 / a0, a1: a1 / a0, a2: a2 / a0 }
    }

    /// Magnitude response in dB at `freq`; used for UI curves and tests.
    pub fn magnitude_db(&self, sample_rate: f32, freq: f32) -> f32 {
        let w = 2.0 * PI * freq as f64 / sample_rate as f64;
        let (s1, c1) = (-w).sin_cos();
        let (s2, c2) = (-2.0 * w).sin_cos();
        let nr = self.b0 + self.b1 * c1 + self.b2 * c2;
        let ni = self.b1 * s1 + self.b2 * s2;
        let dr = 1.0 + self.a1 * c1 + self.a2 * c2;
        let di = self.a1 * s1 + self.a2 * s2;
        let mag = ((nr * nr + ni * ni) / (dr * dr + di * di)).sqrt();
        (20.0 * mag.log10()) as f32
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Biquad {
    c: BiquadCoeffs,
    z1: f64,
    z2: f64,
}

impl Default for Biquad {
    fn default() -> Self {
        Self { c: BiquadCoeffs::IDENTITY, z1: 0.0, z2: 0.0 }
    }
}

impl Biquad {
    pub fn set_coeffs(&mut self, c: BiquadCoeffs) {
        self.c = c;
    }

    pub fn reset(&mut self) {
        self.z1 = 0.0;
        self.z2 = 0.0;
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        let x = x as f64;
        let y = self.c.b0 * x + self.z1;
        self.z1 = self.c.b1 * x - self.c.a1 * y + self.z2;
        self.z2 = self.c.b2 * x - self.c.a2 * y;
        y as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FS: f32 = 48_000.0;

    #[test]
    fn peak_hits_requested_gain_at_center() {
        let c = BiquadCoeffs::new(FilterKind::Peak, FS, 1000.0, 1.0, 6.0);
        assert!((c.magnitude_db(FS, 1000.0) - 6.0).abs() < 0.01);
        assert!(c.magnitude_db(FS, 50.0).abs() < 0.3);
    }

    #[test]
    fn shelves_reach_gain_far_from_corner() {
        let ls = BiquadCoeffs::new(FilterKind::LowShelf, FS, 200.0, 0.707, 9.0);
        assert!((ls.magnitude_db(FS, 20.0) - 9.0).abs() < 0.3);
        assert!(ls.magnitude_db(FS, 10_000.0).abs() < 0.3);
        let hs = BiquadCoeffs::new(FilterKind::HighShelf, FS, 6000.0, 0.707, -9.0);
        assert!((hs.magnitude_db(FS, 20_000.0) + 9.0).abs() < 0.5);
        assert!(hs.magnitude_db(FS, 100.0).abs() < 0.3);
    }

    #[test]
    fn lowpass_and_highpass_attenuate() {
        let lp = BiquadCoeffs::new(FilterKind::LowPass, FS, 1000.0, 0.707, 0.0);
        assert!(lp.magnitude_db(FS, 10_000.0) < -30.0);
        assert!(lp.magnitude_db(FS, 100.0).abs() < 0.2);
        let hp = BiquadCoeffs::new(FilterKind::HighPass, FS, 1000.0, 0.707, 0.0);
        assert!(hp.magnitude_db(FS, 100.0) < -30.0);
    }

    #[test]
    fn time_domain_matches_analytic_response() {
        let c = BiquadCoeffs::new(FilterKind::Peak, FS, 1000.0, 1.0, 6.0);
        let mut f = Biquad::default();
        f.set_coeffs(c);
        let mut peak = 0f32;
        for n in 0..48_000 {
            let x = (2.0 * std::f32::consts::PI * 1000.0 * n as f32 / FS).sin();
            let y = f.process(x);
            if n > 24_000 {
                peak = peak.max(y.abs());
            }
        }
        let db = 20.0 * peak.log10();
        assert!((db - 6.0).abs() < 0.1, "measured {db} dB");
    }

    #[test]
    fn identity_is_transparent() {
        let mut f = Biquad::default();
        for x in [0.0, 0.5, -1.0, 0.25] {
            assert_eq!(f.process(x), x);
        }
    }
}
