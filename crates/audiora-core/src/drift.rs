//! Clock-domain bridge. A producer thread (device A, rate `in_rate`) writes
//! interleaved stereo frames into a ring; the consumer (device B, rate
//! `out_rate`) reads through a 4-point Catmull-Rom interpolator whose ratio is
//! trimmed slowly so the ring fill level stays at its target. That absorbs both
//! the nominal rate difference and the few-hundred-ppm drift between two
//! unsynchronised audio clocks, with no clicks and no growing latency.

use crate::ring::Consumer;

const CHUNK_FRAMES: usize = 4096;
/// Max ratio trim: +-0.5 % (about +-8 cents) — only reached under heavy drift.
const MAX_TRIM: f64 = 0.005;
/// Proportional gain on the filtered fill error (fraction of target).
const TRIM_GAIN: f64 = 0.02;
/// Fill-level smoothing per call (~1 s at 10-20 ms callbacks).
const FILL_ALPHA: f64 = 0.02;
/// After an underrun the fill target grows by this factor (jitter buffer learns the producer's bursts).
const GROW: f64 = 1.5;
/// The target never grows past this multiple of the configured one.
const MAX_GROWTH: f64 = 6.0;
/// Seconds of clean playback before the target eases back toward the configured one.
const RELAX_AFTER_S: f64 = 60.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FillReport {
    /// The ring ran dry; the rest of the block is silence until it re-primes.
    pub underrun: bool,
    /// The ring overfilled and old audio was discarded.
    pub dropped: bool,
}

pub struct DriftResampler {
    nominal: f64,
    ratio: f64,
    frac: f64,
    hist: [[f32; 2]; 4],
    chunk: Vec<f32>,
    cpos: usize,
    clen: usize,
    target: f64,
    base_target: f64,
    out_rate: f64,
    clean_frames: f64,
    /// Seconds of clean playback before relaxing; doubles whenever a relax led to a new underrun.
    relax_s: f64,
    relaxed: bool,
    fill_avg: f64,
    have_avg: bool,
    primed: bool,
}

impl DriftResampler {
    /// `target_frames`: desired ring fill, in input frames.
    pub fn new(in_rate: u32, out_rate: u32, target_frames: usize) -> Self {
        let nominal = in_rate as f64 / out_rate as f64;
        Self {
            nominal,
            ratio: nominal,
            frac: 0.0,
            hist: [[0.0; 2]; 4],
            chunk: vec![0.0; CHUNK_FRAMES * 2],
            cpos: 0,
            clen: 0,
            target: target_frames.max(16) as f64,
            base_target: target_frames.max(16) as f64,
            out_rate: out_rate.max(1) as f64,
            clean_frames: 0.0,
            relax_s: RELAX_AFTER_S,
            relaxed: false,
            fill_avg: 0.0,
            have_avg: false,
            primed: false,
        }
    }

    pub fn is_primed(&self) -> bool {
        self.primed
    }

    /// Current effective input/output ratio (nominal plus drift trim).
    pub fn ratio(&self) -> f64 {
        self.ratio
    }

    /// Current fill target in input frames (grows after underruns, relaxes when stable).
    pub fn target_frames(&self) -> usize {
        self.target as usize
    }

    /// An underrun means the producer delivers in bigger bursts than the buffer allows for:
    /// give it more room (bounded) so the next burst does not run dry.
    fn grow(&mut self) {
        self.target = (self.target * GROW).min(self.base_target * MAX_GROWTH);
        self.clean_frames = 0.0;
        if self.relaxed {
            // the smaller buffer was too small after all: wait much longer before trying again
            self.relax_s = (self.relax_s * 2.0).min(3600.0);
            self.relaxed = false;
        }
    }

    fn relax(&mut self, out_frames: usize) {
        if self.target <= self.base_target {
            return;
        }
        self.clean_frames += out_frames as f64;
        if self.clean_frames >= self.out_rate * self.relax_s {
            self.clean_frames = 0.0;
            self.relaxed = true;
            self.target = (self.target * 0.85).max(self.base_target);
        }
    }

    /// Input frames waiting in the ring plus the internal read-ahead chunk.
    pub fn buffered_frames(&self, cons: &Consumer) -> usize {
        cons.available() / 2 + (self.clen - self.cpos) / 2
    }

    fn next_frame(&mut self, cons: &mut Consumer) -> Option<[f32; 2]> {
        if self.cpos >= self.clen {
            self.clen = cons.pop_slice(&mut self.chunk[..]) / 2 * 2;
            self.cpos = 0;
            if self.clen == 0 {
                return None;
            }
        }
        let f = [self.chunk[self.cpos], self.chunk[self.cpos + 1]];
        self.cpos += 2;
        Some(f)
    }

    fn discard(&mut self, cons: &mut Consumer, mut frames: usize) {
        while frames > 0 {
            if self.next_frame(cons).is_none() {
                break;
            }
            frames -= 1;
        }
    }

    /// Fills `out` (interleaved stereo) completely: audio when primed,
    /// silence while priming or after an underrun.
    pub fn fill(&mut self, cons: &mut Consumer, out: &mut [f32]) -> FillReport {
        let mut report = FillReport::default();
        let buffered = self.buffered_frames(cons) as f64;

        if !self.primed {
            if buffered >= self.target {
                self.primed = true;
                self.have_avg = false;
                let first = self.next_frame(cons).unwrap_or([0.0; 2]);
                self.hist = [first; 4];
                self.frac = 0.0;
            } else {
                out.fill(0.0);
                return report;
            }
        }

        if buffered > self.target * 4.0 {
            let excess = (buffered - self.target) as usize;
            self.discard(cons, excess);
            report.dropped = true;
            self.have_avg = false;
        }

        let buffered = self.buffered_frames(cons) as f64;
        self.fill_avg = if self.have_avg {
            self.fill_avg + (buffered - self.fill_avg) * FILL_ALPHA
        } else {
            self.have_avg = true;
            buffered
        };
        let err = (self.fill_avg - self.target) / self.target;
        self.ratio = self.nominal * (1.0 + (err * TRIM_GAIN).clamp(-MAX_TRIM, MAX_TRIM));

        let mut i = 0;
        while i < out.len() / 2 {
            let t = self.frac as f32;
            for ch in 0..2 {
                let (p0, p1, p2, p3) = (self.hist[0][ch], self.hist[1][ch], self.hist[2][ch], self.hist[3][ch]);
                out[i * 2 + ch] = 0.5
                    * (2.0 * p1
                        + (p2 - p0) * t
                        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t * t
                        + (3.0 * p1 - p0 - 3.0 * p2 + p3) * t * t * t);
            }
            i += 1;
            self.frac += self.ratio;
            while self.frac >= 1.0 {
                self.frac -= 1.0;
                match self.next_frame(cons) {
                    Some(f) => {
                        self.hist = [self.hist[1], self.hist[2], self.hist[3], f];
                    }
                    None => {
                        self.primed = false;
                        report.underrun = true;
                        self.grow();
                        out[i * 2..].fill(0.0);
                        return report;
                    }
                }
            }
        }
        self.relax(out.len() / 2);
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ring::ring;
    use std::f64::consts::TAU;

    fn sine_frames(start: usize, n: usize, rate: f64, hz: f64) -> Vec<f32> {
        (start..start + n)
            .flat_map(|k| {
                let s = (TAU * hz * k as f64 / rate).sin() as f32 * 0.5;
                [s, s]
            })
            .collect()
    }

    /// Simulates a producer whose clock runs `ppm` fast/slow versus the consumer.
    fn simulate(in_rate: u32, out_rate: u32, ppm: f64, seconds: f64) -> (Vec<f32>, usize, usize, f64) {
        let (mut p, mut c) = ring(1 << 16);
        let target = (in_rate as f64 * 0.03) as usize;
        let mut rs = DriftResampler::new(in_rate, out_rate, target);
        let block = 480usize; // output frames per consumer callback
        let actual_in = in_rate as f64 * (1.0 + ppm * 1e-6);
        let mut produced = 0usize;
        let mut owed = 0.0f64;
        let mut out_all = Vec::new();
        let (mut under, mut drops) = (0, 0);
        let callbacks = (seconds * out_rate as f64 / block as f64) as usize;
        for _ in 0..callbacks {
            // producer delivers what its (drifting) clock produced during this period
            owed += block as f64 * actual_in / out_rate as f64;
            let n = owed as usize;
            owed -= n as f64;
            let data = sine_frames(produced, n, in_rate as f64, 1000.0);
            assert_eq!(p.push_slice(&data), data.len());
            produced += n;
            let mut out = vec![0f32; block * 2];
            let r = rs.fill(&mut c, &mut out);
            under += r.underrun as usize;
            drops += r.dropped as usize;
            out_all.extend_from_slice(&out);
        }
        let fill = rs.buffered_frames(&c) as f64 / target as f64;
        (out_all, under, drops, fill)
    }

    fn max_second_difference(out: &[f32]) -> f32 {
        let l: Vec<f32> = out.chunks_exact(2).map(|f| f[0]).collect();
        l.windows(3).map(|w| (w[2] - 2.0 * w[1] + w[0]).abs()).fold(0.0, f32::max)
    }

    #[test]
    fn same_rate_zero_drift_is_clean() {
        let (out, under, drops, _) = simulate(48_000, 48_000, 0.0, 5.0);
        assert_eq!((under, drops), (0, 0));
        // 1 kHz at 48 kHz, amp 0.5: max |second difference| = 0.5 * (2*sin(pi*1000/48000))^2 ~ 0.0086
        let skip = 48_000 * 2; // priming silence + settle
        assert!(max_second_difference(&out[skip..]) < 0.0100, "glitch in output");
    }

    #[test]
    fn rate_conversion_44100_to_48000_keeps_pitch() {
        let (out, under, _, _) = simulate(44_100, 48_000, 0.0, 5.0);
        assert_eq!(under, 0);
        let l: Vec<f32> = out.chunks_exact(2).map(|f| f[0]).skip(48_000 * 2).collect();
        let zc = l.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count();
        let hz = zc as f64 * 48_000.0 / l.len() as f64;
        assert!((hz - 1000.0).abs() < 3.0, "pitch {hz}");
        assert!(max_second_difference(&out[48_000 * 2..]) < 0.0100);
    }

    #[test]
    fn tracks_clock_drift_without_underruns_or_growth() {
        for ppm in [-400.0, 150.0, 400.0] {
            let (out, under, drops, fill) = simulate(48_000, 48_000, ppm, 60.0);
            assert_eq!(under, 0, "{ppm} ppm underran");
            assert_eq!(drops, 0, "{ppm} ppm dropped");
            assert!((0.3..2.5).contains(&fill), "{ppm} ppm: fill ratio {fill}");
            assert!(max_second_difference(&out[48_000 * 2..]) < 0.0100, "{ppm} ppm: discontinuity");
        }
    }

    #[test]
    fn underrun_recovers_after_repriming() {
        let (mut p, mut c) = ring(1 << 14);
        let mut rs = DriftResampler::new(48_000, 48_000, 480);
        let mut out = vec![0f32; 480 * 2];
        p.push_slice(&sine_frames(0, 1000, 48_000.0, 1000.0));
        let r = rs.fill(&mut c, &mut out);
        assert!(!r.underrun && rs.is_primed());
        let mut saw_underrun = false;
        for _ in 0..5 {
            saw_underrun |= rs.fill(&mut c, &mut out).underrun;
        }
        assert!(saw_underrun && !rs.is_primed());
        // silence while starved, then audio again once refilled
        assert!(out.iter().all(|&s| s == 0.0));
        p.push_slice(&sine_frames(0, 2000, 48_000.0, 1000.0));
        rs.fill(&mut c, &mut out);
        assert!(rs.is_primed());
    }

    #[test]
    fn overflow_drops_old_audio_back_to_target() {
        let (mut p, mut c) = ring(1 << 16);
        let mut rs = DriftResampler::new(48_000, 48_000, 480);
        p.push_slice(&sine_frames(0, 480, 48_000.0, 500.0));
        let mut out = vec![0f32; 480 * 2];
        rs.fill(&mut c, &mut out);
        p.push_slice(&sine_frames(0, 20_000, 48_000.0, 500.0));
        let r = rs.fill(&mut c, &mut out);
        assert!(r.dropped);
        assert!(c.available() / 2 < 480 * 3);
    }

    #[test]
    fn bursty_producer_teaches_the_buffer_to_grow_then_stays_clean() {
        // a Bluetooth-like producer: 1920 frames every 4th 480-frame callback
        let (mut p, mut c) = ring(1 << 16);
        let mut rs = DriftResampler::new(48_000, 48_000, 480);
        let mut out = vec![0f32; 480 * 2];
        let (mut early, mut late) = (0, 0);
        for k in 0..4000usize {
            if k % 4 == 0 {
                p.push_slice(&sine_frames(k * 480, 1920, 48_000.0, 1000.0));
            }
            let r = rs.fill(&mut c, &mut out);
            if k < 400 { early += r.underrun as usize } else { late += r.underrun as usize }
        }
        assert!(early > 0, "the small start target should run dry at least once");
        assert_eq!(late, 0, "after growing, bursts must no longer underrun");
        assert!(rs.target_frames() <= 480 * 6);
    }
}
