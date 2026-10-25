//! Strip -> bus mixer. `process` never allocates, locks or blocks.
//!
//! Strip chain:  boost -> denoise -> gate -> EQ -> smart volume -> compressor -> fader/balance -> buses
//! Bus chain:    sum -> fader -> limiter -> output

use std::sync::atomic::Ordering;
use std::sync::Arc;

use audiora_dsp::{
    db_to_lin, Biquad, BiquadCoeffs, Compressor, Denoiser, FilterKind, Gate, Leveler, Limiter,
    DENOISE_SAMPLE_RATE,
};

use crate::params::{MixerParams, StripParams, EQ_BANDS};

/// Longest block handled in one pass; longer inputs are chunked internally.
pub const MAX_BLOCK_FRAMES: usize = 4096;

const BAND_KINDS: [FilterKind; EQ_BANDS] = [
    FilterKind::LowShelf,
    FilterKind::Peak,
    FilterKind::Peak,
    FilterKind::Peak,
    FilterKind::Peak,
    FilterKind::HighShelf,
];

struct StripState {
    denoiser: Option<Box<Denoiser>>,
    gate: Gate,
    comp: Compressor,
    leveler: Leveler,
    gate_on: bool,
    comp_on: bool,
    smart_on: bool,
    eq: [[Biquad; 2]; EQ_BANDS],
    /// (freq, gain_db, q) the filters were last configured with.
    eq_cache: [(f32, f32, f32); EQ_BANDS],
    eq_active: bool,
    gain_l: f32,
    gain_r: f32,
}

struct BusState {
    gain: f32,
    limiter: Box<Limiter>,
    limiter_on: bool,
}

pub struct Mixer {
    sample_rate: f32,
    params: Arc<MixerParams>,
    strips: Vec<StripState>,
    buses: Vec<BusState>,
    scratch: Vec<f32>,
}

impl Mixer {
    pub fn new(sample_rate: f32, strips: usize, buses: usize) -> (Self, Arc<MixerParams>) {
        let params = Arc::new(MixerParams::new(strips, buses));
        let denoise_ok = sample_rate as u32 == DENOISE_SAMPLE_RATE;
        let mixer = Self {
            sample_rate,
            params: params.clone(),
            strips: (0..strips)
                .map(|_| StripState {
                    denoiser: denoise_ok.then(|| Box::new(Denoiser::new())),
                    gate: Gate::new(sample_rate),
                    comp: Compressor::new(sample_rate),
                    leveler: Leveler::new(sample_rate),
                    gate_on: false,
                    comp_on: false,
                    smart_on: false,
                    eq: [[Biquad::default(); 2]; EQ_BANDS],
                    eq_cache: [(f32::NAN, 0.0, 0.0); EQ_BANDS],
                    eq_active: false,
                    gain_l: 1.0,
                    gain_r: 1.0,
                })
                .collect(),
            buses: (0..buses)
                .map(|_| BusState {
                    gain: 1.0,
                    limiter: Box::new(Limiter::new(sample_rate)),
                    limiter_on: false,
                })
                .collect(),
            scratch: vec![0.0; MAX_BLOCK_FRAMES * 2],
        };
        (mixer, params)
    }

    /// `inputs[i]` is strip i, `outputs[b]` is bus b; all interleaved stereo
    /// f32 with the same length. Outputs are overwritten.
    pub fn process(&mut self, inputs: &[&[f32]], outputs: &mut [&mut [f32]]) {
        debug_assert_eq!(inputs.len(), self.strips.len());
        debug_assert_eq!(outputs.len(), self.buses.len());
        let total_frames = inputs.first().map_or(0, |s| s.len() / 2);
        let mut peaks_s = [[0f32; 2]; 64];
        let mut peaks_b = [[0f32; 2]; 32];

        let mut offset = 0;
        while offset < total_frames {
            let frames = (total_frames - offset).min(MAX_BLOCK_FRAMES);
            let range = offset * 2..(offset + frames) * 2;
            self.process_chunk(inputs, outputs, range, frames, &mut peaks_s, &mut peaks_b);
            offset += frames;
        }

        for (i, p) in self.params.strips.iter().enumerate().take(64) {
            p.peak_l.set(peaks_s[i][0]);
            p.peak_r.set(peaks_s[i][1]);
        }
        for (i, p) in self.params.buses.iter().enumerate() {
            p.peak_l.set(peaks_b[i][0]);
            p.peak_r.set(peaks_b[i][1]);
        }
    }

    fn process_chunk(
        &mut self,
        inputs: &[&[f32]],
        outputs: &mut [&mut [f32]],
        range: std::ops::Range<usize>,
        frames: usize,
        peaks_s: &mut [[f32; 2]; 64],
        peaks_b: &mut [[f32; 2]; 32],
    ) {
        for out in outputs.iter_mut() {
            out[range.clone()].fill(0.0);
        }
        let any_solo = self.params.strips.iter().any(|s| s.solo.load(Ordering::Relaxed));
        let inv = 1.0 / frames as f32;

        for (i, (st, p)) in self.strips.iter_mut().zip(&self.params.strips).enumerate() {
            let silenced = p.mute.load(Ordering::Relaxed)
                || (any_solo && !p.solo.load(Ordering::Relaxed));
            let gain = if silenced { 0.0 } else { db_to_lin(p.gain_db.get()) };
            let bal = p.balance.get().clamp(-1.0, 1.0);
            let target_l = gain * (1.0 - bal).min(1.0);
            let target_r = gain * (1.0 + bal).min(1.0);
            let mask = p.bus_mask.load(Ordering::Relaxed);

            if st.gain_l == 0.0 && st.gain_r == 0.0 && target_l == 0.0 && target_r == 0.0 {
                continue;
            }

            let buf = &mut self.scratch[..frames * 2];
            buf.copy_from_slice(&inputs[i][range.clone()]);

            let boost = p.boost_db.get();
            if boost > 0.01 {
                let g = db_to_lin(boost);
                for v in buf.iter_mut() {
                    *v *= g;
                }
            }

            if p.denoise.load(Ordering::Relaxed) {
                if let Some(d) = st.denoiser.as_mut() {
                    d.process(buf);
                }
            }

            if p.gate.enabled.load(Ordering::Relaxed) {
                let open = st.gate.process(buf, &p.gate.settings());
                p.gate.open.set(open as u8 as f32);
                st.gate_on = true;
            } else if st.gate_on {
                st.gate.reset();
                p.gate.open.set(1.0);
                st.gate_on = false;
            }

            update_eq(st, p, self.sample_rate);
            if st.eq_active {
                for f in buf.chunks_exact_mut(2) {
                    for band in st.eq.iter_mut() {
                        f[0] = band[0].process(f[0]);
                        f[1] = band[1].process(f[1]);
                    }
                }
            }

            if p.smart.enabled.load(Ordering::Relaxed) {
                p.smart.gain_db.set(st.leveler.process(buf, p.smart.target_db.get()));
                st.smart_on = true;
            } else if st.smart_on {
                st.leveler.reset();
                p.smart.gain_db.set(0.0);
                st.smart_on = false;
            }

            if p.comp.enabled.load(Ordering::Relaxed) {
                p.comp.gr_db.set(st.comp.process(buf, &p.comp.settings()));
                st.comp_on = true;
            } else if st.comp_on {
                st.comp.reset();
                p.comp.gr_db.set(0.0);
                st.comp_on = false;
            }

            let step_l = (target_l - st.gain_l) * inv;
            let step_r = (target_r - st.gain_r) * inv;
            let (mut gl, mut gr) = (st.gain_l, st.gain_r);
            let (mut pl, mut pr) = (peaks_s[i][0], peaks_s[i][1]);
            for f in buf.chunks_exact_mut(2) {
                gl += step_l;
                gr += step_r;
                f[0] *= gl;
                f[1] *= gr;
                pl = pl.max(f[0].abs());
                pr = pr.max(f[1].abs());
            }
            peaks_s[i] = [pl, pr];
            st.gain_l = target_l;
            st.gain_r = target_r;

            for (b, out) in outputs.iter_mut().enumerate() {
                if mask & (1 << b) != 0 {
                    for (o, s) in out[range.clone()].iter_mut().zip(buf.iter()) {
                        *o += *s;
                    }
                }
            }
        }

        for (b, (st, p)) in self.buses.iter_mut().zip(&self.params.buses).enumerate() {
            let target = if p.mute.load(Ordering::Relaxed) { 0.0 } else { db_to_lin(p.gain_db.get()) };
            let step = (target - st.gain) * inv;
            let mut g = st.gain;
            let out = &mut outputs[b][range.clone()];
            for f in out.chunks_exact_mut(2) {
                g += step;
                f[0] *= g;
                f[1] *= g;
            }
            st.gain = target;

            if p.limiter.enabled.load(Ordering::Relaxed) {
                p.limiter.gr_db.set(st.limiter.process(out, &p.limiter.settings()));
                st.limiter_on = true;
            } else if st.limiter_on {
                st.limiter.reset();
                p.limiter.gr_db.set(0.0);
                st.limiter_on = false;
            }

            let (mut pl, mut pr) = (peaks_b[b][0], peaks_b[b][1]);
            for f in out.chunks_exact(2) {
                pl = pl.max(f[0].abs());
                pr = pr.max(f[1].abs());
            }
            peaks_b[b] = [pl, pr];
        }
    }
}

fn update_eq(st: &mut StripState, p: &StripParams, fs: f32) {
    let mut active = false;
    for band in 0..EQ_BANDS {
        let cur = (p.eq[band].freq.get(), p.eq[band].gain_db.get(), p.eq[band].q.get());
        if cur != st.eq_cache[band] {
            let c = if cur.1.abs() < 0.01 {
                BiquadCoeffs::IDENTITY
            } else {
                BiquadCoeffs::new(BAND_KINDS[band], fs, cur.0, cur.2, cur.1)
            };
            st.eq[band][0].set_coeffs(c);
            st.eq[band][1].set_coeffs(c);
            st.eq_cache[band] = cur;
        }
        active |= cur.1.abs() >= 0.01;
    }
    st.eq_active = active;
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAMES: usize = 512;

    fn run(mixer: &mut Mixer, inputs: &[Vec<f32>], buses: usize) -> Vec<Vec<f32>> {
        let refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
        let mut outs = vec![vec![0f32; FRAMES * 2]; buses];
        let mut mut_refs: Vec<&mut [f32]> = outs.iter_mut().map(|v| v.as_mut_slice()).collect();
        // two passes so gain ramps settle
        mixer.process(&refs, &mut mut_refs);
        mixer.process(&refs, &mut mut_refs);
        outs
    }

    fn dc(v: f32) -> Vec<f32> {
        vec![v; FRAMES * 2]
    }

    fn sine(amp: f32, hz: f32) -> Vec<f32> {
        (0..FRAMES)
            .flat_map(|n| {
                let s = amp * (2.0 * std::f32::consts::PI * hz * n as f32 / 48_000.0).sin();
                [s, s]
            })
            .collect()
    }

    #[test]
    fn unity_passthrough() {
        let (mut m, _p) = Mixer::new(48_000.0, 1, 1);
        let out = run(&mut m, &[dc(0.5)], 1);
        assert!(out[0].iter().all(|&s| (s - 0.5).abs() < 1e-6));
    }

    #[test]
    fn gain_balance_and_bus_gain() {
        let (mut m, p) = Mixer::new(48_000.0, 1, 1);
        p.strips[0].gain_db.set(-6.0206);
        p.strips[0].balance.set(1.0); // hard right: left silent
        p.buses[0].gain_db.set(6.0206);
        let out = run(&mut m, &[dc(0.5)], 1);
        let last = out[0].len() - 2;
        assert!(out[0][last].abs() < 1e-6);
        assert!((out[0][last + 1] - 0.5).abs() < 1e-3);
    }

    #[test]
    fn mute_and_solo() {
        let (mut m, p) = Mixer::new(48_000.0, 2, 1);
        p.strips[1].mute.store(true, Ordering::Relaxed);
        let out = run(&mut m, &[dc(0.25), dc(0.25)], 1);
        assert!((out[0][100] - 0.25).abs() < 1e-6);

        p.strips[1].mute.store(false, Ordering::Relaxed);
        p.strips[1].solo.store(true, Ordering::Relaxed);
        let out = run(&mut m, &[dc(0.25), dc(0.5)], 1);
        assert!((out[0][100] - 0.5).abs() < 1e-6, "only soloed strip is audible");
    }

    #[test]
    fn bus_routing_mask() {
        let (mut m, p) = Mixer::new(48_000.0, 2, 2);
        p.strips[0].bus_mask.store(0b01, Ordering::Relaxed);
        p.strips[1].bus_mask.store(0b11, Ordering::Relaxed);
        let out = run(&mut m, &[dc(0.25), dc(0.5)], 2);
        assert!((out[0][100] - 0.75).abs() < 1e-6);
        assert!((out[1][100] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn gain_changes_are_ramped_not_stepped() {
        let (mut m, p) = Mixer::new(48_000.0, 1, 1);
        let inputs = [dc(1.0)];
        let refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
        let mut out = [vec![0f32; FRAMES * 2]];
        let mut r: Vec<&mut [f32]> = out.iter_mut().map(|v| v.as_mut_slice()).collect();
        m.process(&refs, &mut r);
        p.strips[0].gain_db.set(-120.0);
        m.process(&refs, &mut r);
        let max_step = r[0].chunks_exact(2).collect::<Vec<_>>().windows(2)
            .map(|w| (w[0][0] - w[1][0]).abs()).fold(0f32, f32::max);
        assert!(max_step < 0.01, "zipper step {max_step}");
        assert!(r[0][FRAMES * 2 - 2].abs() < 0.01);
    }

    #[test]
    fn eq_boost_changes_signal_and_peaks_are_reported() {
        let (mut m, p) = Mixer::new(48_000.0, 1, 1);
        p.strips[0].eq[1].gain_db.set(12.0);
        p.strips[0].eq[1].freq.set(1000.0);
        let input = sine(0.1, 1000.0);
        let mut last_peak = 0.0;
        for _ in 0..20 {
            run(&mut m, std::slice::from_ref(&input), 1);
            last_peak = p.strips[0].peak_l.get();
        }
        assert!(last_peak > 0.3, "expected ~0.4 after +12 dB, got {last_peak}");
    }

    #[test]
    fn gate_silences_quiet_strip_and_reports_closed() {
        let (mut m, p) = Mixer::new(44_100.0, 1, 1); // 44.1k: denoise unavailable, gate still works
        p.strips[0].gate.enabled.store(true, Ordering::Relaxed);
        p.strips[0].gate.threshold_db.set(-30.0);
        p.strips[0].gate.release_ms.set(5.0);
        let quiet = sine(0.003, 440.0); // about -50 dBFS
        for _ in 0..40 {
            run(&mut m, std::slice::from_ref(&quiet), 1);
        }
        assert_eq!(p.strips[0].gate.open.get(), 0.0);
        assert!(p.strips[0].peak_l.get() < 0.0005);
    }

    #[test]
    fn compressor_reports_gain_reduction() {
        let (mut m, p) = Mixer::new(48_000.0, 1, 1);
        p.strips[0].comp.enabled.store(true, Ordering::Relaxed);
        p.strips[0].comp.threshold_db.set(-20.0);
        p.strips[0].comp.ratio.set(4.0);
        p.strips[0].comp.attack_ms.set(1.0);
        p.strips[0].comp.knee_db.set(0.0);
        let loud = sine(0.5, 1000.0);
        for _ in 0..40 {
            run(&mut m, std::slice::from_ref(&loud), 1);
        }
        let gr = p.strips[0].comp.gr_db.get();
        assert!(gr > 8.0 && gr < 12.0, "gr {gr}");
    }

    #[test]
    fn bus_limiter_caps_output() {
        let (mut m, p) = Mixer::new(48_000.0, 1, 1);
        p.buses[0].limiter.enabled.store(true, Ordering::Relaxed);
        p.buses[0].limiter.ceiling_db.set(-6.0);
        let hot = sine(1.5, 200.0);
        let mut worst = 0f32;
        for _ in 0..30 {
            let out = run(&mut m, std::slice::from_ref(&hot), 1);
            worst = worst.max(out[0].iter().fold(0.0, |a, &b| a.max(b.abs())));
        }
        assert!(worst <= db_to_lin(-6.0) + 1e-3, "worst {worst}");
        assert!(p.buses[0].limiter.gr_db.get() > 1.0);
    }

    #[test]
    fn denoise_flag_is_inert_at_unsupported_rates_and_adds_latency_at_48k() {
        let (mut m, p) = Mixer::new(44_100.0, 1, 1);
        p.strips[0].denoise.store(true, Ordering::Relaxed);
        let out = run(&mut m, &[dc(0.5)], 1);
        assert!(out[0].iter().all(|&s| (s - 0.5).abs() < 1e-6), "bypassed at 44.1k");

        let (mut m, p) = Mixer::new(48_000.0, 1, 1);
        p.strips[0].denoise.store(true, Ordering::Relaxed);
        let out = run(&mut m, &[dc(0.5)], 1);
        assert!(out[0][..100].iter().all(|&s| s.abs() < 1e-3), "10 ms of latency, silent first");
    }

    #[test]
    fn boost_adds_gain_before_the_chain() {
        let (mut m, p) = Mixer::new(48_000.0, 1, 1);
        p.strips[0].boost_db.set(6.0206);
        let out = run(&mut m, &[dc(0.25)], 1);
        assert!((out[0][100] - 0.5).abs() < 1e-3);
    }

    #[test]
    fn smart_volume_levels_a_loud_strip_down() {
        let (mut m, p) = Mixer::new(48_000.0, 1, 1);
        p.strips[0].smart.enabled.store(true, Ordering::Relaxed);
        p.strips[0].smart.target_db.set(-24.0);
        let loud = sine(0.7, 1000.0);
        for _ in 0..400 {
            run(&mut m, std::slice::from_ref(&loud), 1);
        }
        assert!(p.strips[0].smart.gain_db.get() < -4.0, "gain {}", p.strips[0].smart.gain_db.get());
    }

    #[test]
    fn disabling_processors_restores_passthrough() {
        let (mut m, p) = Mixer::new(48_000.0, 1, 1);
        p.strips[0].comp.enabled.store(true, Ordering::Relaxed);
        p.strips[0].comp.threshold_db.set(-40.0);
        let loud = sine(0.5, 1000.0);
        for _ in 0..10 {
            run(&mut m, std::slice::from_ref(&loud), 1);
        }
        assert!(p.strips[0].comp.gr_db.get() > 1.0);
        p.strips[0].comp.enabled.store(false, Ordering::Relaxed);
        run(&mut m, std::slice::from_ref(&loud), 1);
        assert_eq!(p.strips[0].comp.gr_db.get(), 0.0);
    }
}
