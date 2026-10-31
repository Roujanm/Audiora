//! Dynamics processors operating on interleaved stereo with linked detection.
//! No allocation after construction; safe on the audio thread.

use crate::{db_to_lin, lin_to_db};

fn time_coeff(ms: f32, sample_rate: f32) -> f32 {
    if ms <= 0.0 {
        0.0
    } else {
        (-1.0 / (ms * 0.001 * sample_rate)).exp()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct GateSettings {
    pub threshold_db: f32,
    pub attack_ms: f32,
    pub release_ms: f32,
    /// Attenuation applied while closed (positive dB, e.g. 60).
    pub range_db: f32,
    /// How long the gate stays open after the level falls below the threshold, so word endings and
    /// short pauses are not chopped.
    pub hold_ms: f32,
    /// Voice focus (dB, 0 = off): the gate learns how loud the voice right at the microphone is
    /// and stays closed for anything more than this much quieter (a voice across the room, a TV,
    /// the next room), whatever the fixed threshold says.
    pub focus_db: f32,
}

impl Default for GateSettings {
    fn default() -> Self {
        Self { threshold_db: -50.0, attack_ms: 2.0, release_ms: 120.0, range_db: 60.0, hold_ms: 80.0, focus_db: 0.0 }
    }
}

/// How the learned voice level follows: it rises within a syllable and sinks slowly (about
/// 1 dB per second), so a pause in speech does not let the room back in at once.
const VOICE_RISE_MS: f32 = 30.0;
const VOICE_FALL_MS: f32 = 8000.0;

/// The gate opens at the threshold but only closes this far below it, so a voice hovering at the
/// threshold does not make it chatter.
const HYSTERESIS_DB: f32 = 4.0;

/// Downward expander/gate with hold, hysteresis and smoothed gain.
pub struct Gate {
    fs: f32,
    env: f32,
    gain: f32,
    open: bool,
    hold_left: u32,
    /// Learned level of the close voice (linear), for voice focus.
    voice: f32,
}

impl Gate {
    pub fn new(sample_rate: f32) -> Self {
        Self { fs: sample_rate, env: 0.0, gain: 1.0, open: true, hold_left: 0, voice: 0.0 }
    }

    /// Returns true when the gate is (mostly) open at the end of the block.
    pub fn process(&mut self, buf: &mut [f32], s: &GateSettings) -> bool {
        let open_at = db_to_lin(s.threshold_db);
        let close_at = db_to_lin(s.threshold_db - HYSTERESIS_DB);
        let hold = (s.hold_ms.max(0.0) * 0.001 * self.fs) as u32;
        let floor = db_to_lin(-s.range_db.abs());
        let att = time_coeff(s.attack_ms, self.fs);
        let rel = time_coeff(s.release_ms, self.fs);
        let env_rel = time_coeff(20.0, self.fs); // detector decay
        let focus = (s.focus_db > 0.0).then(|| db_to_lin(-s.focus_db));
        let (v_rise, v_fall) = (time_coeff(VOICE_RISE_MS, self.fs), time_coeff(VOICE_FALL_MS, self.fs));
        for f in buf.chunks_exact_mut(2) {
            let level = f[0].abs().max(f[1].abs());
            self.env = if level > self.env { level } else { level + (self.env - level) * env_rel };
            // with focus on, the threshold rides `focus_db` under the learned voice level
            let (open_at, close_at) = match focus {
                Some(k) => {
                    self.voice = if self.env > self.voice { self.env + (self.voice - self.env) * v_rise } else { self.voice * v_fall };
                    let t = (self.voice * k).max(open_at);
                    (t, t * (close_at / open_at))
                }
                None => (open_at, close_at),
            };
            if self.env >= open_at {
                self.open = true;
                self.hold_left = hold;
            } else if self.env < close_at {
                if self.hold_left > 0 {
                    self.hold_left -= 1;
                } else {
                    self.open = false;
                }
            }
            let target = if self.open { 1.0 } else { floor };
            let c = if target > self.gain { att } else { rel };
            self.gain = target + (self.gain - target) * c;
            f[0] *= self.gain;
            f[1] *= self.gain;
        }
        self.gain > 0.5
    }

    pub fn reset(&mut self) {
        self.env = 0.0;
        self.gain = 1.0;
        self.open = true;
        self.hold_left = 0;
        self.voice = 0.0;
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CompSettings {
    pub threshold_db: f32,
    pub ratio: f32,
    pub attack_ms: f32,
    pub release_ms: f32,
    pub makeup_db: f32,
    pub knee_db: f32,
}

impl Default for CompSettings {
    fn default() -> Self {
        Self { threshold_db: -18.0, ratio: 3.0, attack_ms: 10.0, release_ms: 120.0, makeup_db: 0.0, knee_db: 6.0 }
    }
}

/// Feed-forward log-domain compressor with soft knee.
pub struct Compressor {
    fs: f32,
    /// Smoothed gain reduction in dB (>= 0).
    gr_db: f32,
}

impl Compressor {
    pub fn new(sample_rate: f32) -> Self {
        Self { fs: sample_rate, gr_db: 0.0 }
    }

    /// Returns the gain reduction (dB, >= 0) at the end of the block.
    pub fn process(&mut self, buf: &mut [f32], s: &CompSettings) -> f32 {
        let ratio = s.ratio.max(1.0);
        let slope = 1.0 - 1.0 / ratio;
        let att = time_coeff(s.attack_ms, self.fs);
        let rel = time_coeff(s.release_ms, self.fs);
        let makeup = db_to_lin(s.makeup_db);
        let half_knee = s.knee_db.max(0.0) * 0.5;
        for f in buf.chunks_exact_mut(2) {
            let level_db = lin_to_db(f[0].abs().max(f[1].abs()));
            let over = level_db - s.threshold_db;
            let target = if half_knee > 0.0 && over.abs() <= half_knee {
                let x = over + half_knee;
                slope * x * x / (4.0 * half_knee)
            } else if over > 0.0 {
                slope * over
            } else {
                0.0
            };
            let c = if target > self.gr_db { att } else { rel };
            self.gr_db = target + (self.gr_db - target) * c;
            let g = db_to_lin(-self.gr_db) * makeup;
            f[0] *= g;
            f[1] *= g;
        }
        self.gr_db
    }

    pub fn reset(&mut self) {
        self.gr_db = 0.0;
    }
}

#[derive(Clone, Copy, Debug)]
pub struct LimiterSettings {
    pub ceiling_db: f32,
    pub release_ms: f32,
}

impl Default for LimiterSettings {
    fn default() -> Self {
        Self { ceiling_db: -1.0, release_ms: 80.0 }
    }
}

const LOOKAHEAD_MS: f32 = 1.5;
const MAX_LOOKAHEAD: usize = 512;

/// Look-ahead brick-wall limiter: sliding-window minimum of the required gain,
/// smoothed by a moving average of the same length so the gain is already
/// down when the offending sample leaves the delay line.
pub struct Limiter {
    fs: f32,
    n: usize,
    delay: [[f32; MAX_LOOKAHEAD]; 2],
    req: [f32; MAX_LOOKAHEAD],
    mins: [f32; MAX_LOOKAHEAD],
    avg_sum: f32,
    pos: usize,
    release_gain: f32,
    gr_db: f32,
}

impl Limiter {
    pub fn new(sample_rate: f32) -> Self {
        let n = ((LOOKAHEAD_MS * 0.001 * sample_rate) as usize).clamp(8, MAX_LOOKAHEAD);
        Self {
            fs: sample_rate,
            n,
            delay: [[0.0; MAX_LOOKAHEAD]; 2],
            req: [1.0; MAX_LOOKAHEAD],
            mins: [1.0; MAX_LOOKAHEAD],
            avg_sum: n as f32,
            pos: 0,
            release_gain: 1.0,
            gr_db: 0.0,
        }
    }

    /// Fixed latency added by the look-ahead, in frames.
    pub fn latency_frames(&self) -> usize {
        self.n - 1
    }

    /// Returns the current gain reduction in dB (>= 0).
    pub fn process(&mut self, buf: &mut [f32], s: &LimiterSettings) -> f32 {
        let ceiling = db_to_lin(s.ceiling_db.min(0.0));
        let rel = time_coeff(s.release_ms, self.fs);
        let n = self.n;
        for f in buf.chunks_exact_mut(2) {
            let peak = f[0].abs().max(f[1].abs());
            let need = if peak > ceiling { ceiling / peak } else { 1.0 };
            // release-limited requirement: recover slowly after a reduction
            self.release_gain = if need < self.release_gain {
                need
            } else {
                need + (self.release_gain - need) * rel
            };
            let required = self.release_gain.min(need);

            // Output is delayed by n-1 frames; the window minimum (n) and the
            // moving average (n) then guarantee gain <= required for every
            // sample that leaves the delay line.
            let p = self.pos;
            self.delay[0][p] = f[0];
            self.delay[1][p] = f[1];
            self.req[p] = required;
            let q = (p + MAX_LOOKAHEAD - (n - 1)) % MAX_LOOKAHEAD;
            let (out_l, out_r) = (self.delay[0][q], self.delay[1][q]);

            // window minimum over the last n requirements
            let mut m = 1.0f32;
            for k in 0..n {
                m = m.min(self.req[(p + MAX_LOOKAHEAD - k) % MAX_LOOKAHEAD]);
            }
            let old = self.mins[(p + MAX_LOOKAHEAD - n) % MAX_LOOKAHEAD];
            self.mins[p] = m;
            self.avg_sum += m - old;
            let g = (self.avg_sum / n as f32).min(1.0);

            f[0] = out_l * g;
            f[1] = out_r * g;
            self.gr_db = -lin_to_db(g);
            self.pos = (p + 1) % MAX_LOOKAHEAD;
        }
        self.gr_db
    }

    pub fn reset(&mut self) {
        *self = Self::new(self.fs);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    const FS: f32 = 48_000.0;

    fn sine(frames: usize, amp: f32, hz: f32) -> Vec<f32> {
        (0..frames)
            .flat_map(|n| {
                let s = amp * (TAU * hz * n as f32 / FS).sin();
                [s, s]
            })
            .collect()
    }

    fn peak(buf: &[f32]) -> f32 {
        buf.iter().fold(0.0, |a, &b| a.max(b.abs()))
    }

    #[test]
    fn gate_closes_on_quiet_and_passes_loud() {
        let mut g = Gate::new(FS);
        let s = GateSettings { threshold_db: -40.0, ..Default::default() };
        let mut loud = sine(4800, 0.5, 440.0);
        g.process(&mut loud, &s);
        assert!(peak(&loud[2400..]) > 0.45, "loud signal must pass");
        let mut quiet = sine(48_000, 0.001, 440.0); // -60 dBFS
        let open = g.process(&mut quiet, &s);
        assert!(!open);
        assert!(peak(&quiet[quiet.len() - 2000..]) < 0.0002, "should be ~60 dB down");
    }

    #[test]
    fn voice_focus_keeps_a_distant_voice_out_but_lets_the_close_voice_in() {
        // a low fixed threshold that a voice across the room (-40 dBFS) clears easily
        let s = GateSettings { threshold_db: -55.0, focus_db: 18.0, ..Default::default() };
        let mut g = Gate::new(FS);
        let mut close = sine(24_000, 0.3, 300.0); // -10 dBFS: talking right at the microphone
        g.process(&mut close, &s);
        assert!(peak(&close[12_000..]) > 0.28, "the close voice must pass");
        let mut far = sine(48_000, 0.01, 300.0); // -40 dBFS, 30 dB quieter
        g.process(&mut far, &s);
        assert!(peak(&far[far.len() - 24_000..]) < 0.0005, "the distant voice must stay out");
        let mut close = sine(9_600, 0.3, 300.0);
        g.process(&mut close, &s);
        assert!(peak(&close[2_400..]) > 0.28, "the close voice opens it again at once");

        // the same distant voice passes with focus off (only the fixed threshold applies)
        let mut g = Gate::new(FS);
        let off = GateSettings { focus_db: 0.0, ..s };
        let mut far = sine(48_000, 0.01, 300.0);
        g.process(&mut far, &off);
        assert!(peak(&far[far.len() - 24_000..]) > 0.009);
    }

    #[test]
    fn compressor_reduces_loud_signal_by_ratio() {
        let mut c = Compressor::new(FS);
        let s = CompSettings { threshold_db: -20.0, ratio: 4.0, attack_ms: 1.0, release_ms: 50.0, makeup_db: 0.0, knee_db: 0.0 };
        let mut x = sine(48_000, 0.5, 1000.0); // -6 dBFS peak: 14 dB over
        let gr = c.process(&mut x, &s);
        // expected GR = 14 * (1 - 1/4) = 10.5 dB
        assert!((gr - 10.5).abs() < 0.5, "gr {gr}");
        let out_db = lin_to_db(peak(&x[x.len() - 4800..]));
        assert!((out_db - (-6.0 - 10.5)).abs() < 1.0, "out {out_db}");
    }

    #[test]
    fn compressor_leaves_quiet_signal_alone() {
        let mut c = Compressor::new(FS);
        let mut x = sine(9600, 0.01, 1000.0);
        let before = x.clone();
        c.process(&mut x, &CompSettings::default());
        assert!(x.iter().zip(&before).all(|(a, b)| (a - b).abs() < 1e-6));
    }

    #[test]
    fn limiter_never_exceeds_ceiling() {
        let mut l = Limiter::new(FS);
        let s = LimiterSettings { ceiling_db: -3.0, release_ms: 50.0 };
        let ceiling = db_to_lin(-3.0);
        let mut x = sine(2000, 0.1, 300.0);
        x.extend(sine(4000, 1.8, 300.0)); // sudden overs
        x.extend(sine(4000, 0.1, 300.0));
        for chunk in x.chunks_mut(512) {
            l.process(chunk, &s);
        }
        assert!(peak(&x) <= ceiling + 1e-4, "peak {} > {ceiling}", peak(&x));
        assert!(peak(&x) > ceiling * 0.9, "limiter should not over-attenuate");
    }

    #[test]
    fn limiter_is_transparent_below_ceiling() {
        let mut l = Limiter::new(FS);
        let original = sine(4800, 0.2, 500.0);
        let mut x = original.clone();
        l.process(&mut x, &LimiterSettings::default());
        let d = l.latency_frames() * 2;
        for i in d..x.len() {
            assert!((x[i] - original[i - d]).abs() < 1e-5);
        }
    }
}
