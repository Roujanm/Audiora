//! Smart Volume: a slow automatic gain that pulls loud passages down and quiet ones up
//! towards a target loudness, so explosions and whispers end up closer together.
//! Interleaved stereo, linked detection, no allocation after construction.

/// Below this level the signal counts as silence: the gain relaxes to 0 dB instead of
/// pumping up the noise floor.
const FLOOR_DB: f32 = -55.0;
const MAX_CUT_DB: f32 = -20.0;
const MAX_BOOST_DB: f32 = 10.0;
/// Frames per gain update; gain is interpolated linearly across each step.
const STEP: usize = 64;

fn coeff(ms: f32, rate: f32) -> f32 {
    (-1.0 / (ms * 0.001 * rate)).exp()
}

pub struct Leveler {
    env: f32,
    gain_db: f32,
    det_rise: f32,
    det_fall: f32,
    cut: f32,
    boost: f32,
}

impl Leveler {
    pub fn new(sample_rate: f32) -> Self {
        let step_rate = sample_rate / STEP as f32;
        Self {
            env: 0.0,
            gain_db: 0.0,
            det_rise: coeff(15.0, sample_rate),
            det_fall: coeff(120.0, sample_rate),
            cut: coeff(40.0, step_rate),
            boost: coeff(900.0, step_rate),
        }
    }

    pub fn reset(&mut self) {
        self.env = 0.0;
        self.gain_db = 0.0;
    }

    /// Levels `buf` towards `target_db` (dBFS, roughly RMS). Returns the gain now applied in dB.
    pub fn process(&mut self, buf: &mut [f32], target_db: f32) -> f32 {
        for block in buf.chunks_mut(STEP * 2) {
            for f in block.chunks_exact(2) {
                let p = (f[0] * f[0]).max(f[1] * f[1]);
                let c = if p > self.env { self.det_rise } else { self.det_fall };
                self.env = p + (self.env - p) * c;
            }
            let level_db = 10.0 * self.env.max(1e-12).log10();
            let want = if level_db < FLOOR_DB { 0.0 } else { (target_db - level_db).clamp(MAX_CUT_DB, MAX_BOOST_DB) };
            let c = if want < self.gain_db { self.cut } else { self.boost };
            let next = want + (self.gain_db - want) * c;
            let (g0, g1) = (10f32.powf(self.gain_db / 20.0), 10f32.powf(next / 20.0));
            let frames = block.len() / 2;
            for (i, f) in block.chunks_exact_mut(2).enumerate() {
                let g = g0 + (g1 - g0) * (i + 1) as f32 / frames as f32;
                f[0] *= g;
                f[1] *= g;
            }
            self.gain_db = next;
        }
        self.gain_db
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(amp: f32, frames: usize) -> Vec<f32> {
        (0..frames)
            .flat_map(|n| {
                let s = amp * (2.0 * std::f32::consts::PI * 440.0 * n as f32 / 48_000.0).sin();
                [s, s]
            })
            .collect()
    }

    fn settle(l: &mut Leveler, amp: f32, target: f32) -> f32 {
        let mut g = 0.0;
        for _ in 0..40 {
            let mut b = sine(amp, 4800);
            g = l.process(&mut b, target);
        }
        g
    }

    #[test]
    fn loud_passages_are_pulled_down() {
        let mut l = Leveler::new(48_000.0);
        let g = settle(&mut l, 0.7, -24.0);
        assert!(g < -6.0, "gain {g}");
    }

    #[test]
    fn quiet_passages_are_lifted_but_capped() {
        let mut l = Leveler::new(48_000.0);
        let g = settle(&mut l, 0.01, -24.0);
        assert!(g > 5.0 && g <= MAX_BOOST_DB + 0.01, "gain {g}");
    }

    #[test]
    fn silence_does_not_pump_the_noise_floor() {
        let mut l = Leveler::new(48_000.0);
        settle(&mut l, 0.01, -24.0);
        let g = settle(&mut l, 0.0001, -24.0);
        assert!(g.abs() < 1.0, "gain {g}");
    }
}
