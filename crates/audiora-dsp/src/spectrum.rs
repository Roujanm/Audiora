//! Real-time spectrum analyser: Hann-windowed 4096-point FFT folded onto 96 log-spaced
//! bands between 20 Hz and 20 kHz, in dBFS (a full-scale sine on a band centre reads 0 dB,
//! within the window scalloping of about 1.4 dB). Meant for an analysis thread.

use std::sync::Arc;

use realfft::num_complex::Complex;
use realfft::{RealFftPlanner, RealToComplex};

pub const SPECTRUM_BANDS: usize = 96;
pub const SPECTRUM_FFT: usize = 4096;
pub const SPECTRUM_FLOOR: f32 = -120.0;

const F_MIN: f32 = 20.0;
const F_MAX: f32 = 20_000.0;

/// Centre frequency of band `i`.
pub fn band_frequency(i: usize) -> f32 {
    F_MIN * (F_MAX / F_MIN).powf(i as f32 / (SPECTRUM_BANDS - 1) as f32)
}

pub struct SpectrumAnalyzer {
    bin_hz: f32,
    window: Vec<f32>,
    amp_scale: f32,
    ring: Vec<f32>,
    pos: usize,
    filled: usize,
    fft: Arc<dyn RealToComplex<f32>>,
    time: Vec<f32>,
    freq: Vec<Complex<f32>>,
    scratch: Vec<Complex<f32>>,
    mag: Vec<f32>,
}

impl SpectrumAnalyzer {
    pub fn new(sample_rate: f32) -> Self {
        let fft = RealFftPlanner::<f32>::new().plan_fft_forward(SPECTRUM_FFT);
        let window: Vec<f32> = (0..SPECTRUM_FFT)
            .map(|n| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * n as f32 / SPECTRUM_FFT as f32).cos())
            .collect();
        let amp_scale = 2.0 / window.iter().sum::<f32>();
        Self {
            bin_hz: sample_rate / SPECTRUM_FFT as f32,
            window,
            amp_scale,
            ring: vec![0.0; SPECTRUM_FFT],
            pos: 0,
            filled: 0,
            time: fft.make_input_vec(),
            freq: fft.make_output_vec(),
            scratch: fft.make_scratch_vec(),
            mag: vec![0.0; SPECTRUM_FFT / 2 + 1],
            fft,
        }
    }

    /// Feeds mono samples; keeps the most recent `SPECTRUM_FFT`.
    pub fn push(&mut self, mono: &[f32]) {
        for &s in mono {
            self.ring[self.pos] = s;
            self.pos = (self.pos + 1) % SPECTRUM_FFT;
        }
        self.filled = (self.filled + mono.len()).min(SPECTRUM_FFT);
    }

    /// True once a full window of audio has arrived.
    pub fn ready(&self) -> bool {
        self.filled >= SPECTRUM_FFT
    }

    /// Writes the current spectrum, one dBFS value per band.
    pub fn compute(&mut self, out: &mut [f32; SPECTRUM_BANDS]) {
        for (i, t) in self.time.iter_mut().enumerate() {
            *t = self.ring[(self.pos + i) % SPECTRUM_FFT] * self.window[i];
        }
        if self.fft.process_with_scratch(&mut self.time, &mut self.freq, &mut self.scratch).is_err() {
            out.fill(SPECTRUM_FLOOR);
            return;
        }
        for (m, c) in self.mag.iter_mut().zip(&self.freq) {
            *m = c.norm() * self.amp_scale;
        }

        let ratio = (F_MAX / F_MIN).powf(1.0 / (SPECTRUM_BANDS - 1) as f32).sqrt();
        let last = self.mag.len() - 1;
        for (i, o) in out.iter_mut().enumerate() {
            let fc = band_frequency(i);
            let (lo, hi) = (fc / ratio / self.bin_hz, fc * ratio / self.bin_hz);
            let amp = if hi - lo < 1.0 {
                // narrower than one bin: interpolate between the neighbours
                let x = fc / self.bin_hz;
                let (i0, frac) = (x.floor() as usize, x.fract());
                self.mag[i0.min(last)] * (1.0 - frac) + self.mag[(i0 + 1).min(last)] * frac
            } else {
                let (a, b) = (lo.ceil() as usize, (hi.floor() as usize).min(last));
                self.mag[a.min(last)..=b.max(a.min(last))].iter().fold(0.0f32, |m, &v| m.max(v))
            };
            *o = if amp <= 1e-6 { SPECTRUM_FLOOR } else { (20.0 * amp.log10()).max(SPECTRUM_FLOOR) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FS: f32 = 48_000.0;

    fn sine(hz: f32, amp: f32, n: usize) -> Vec<f32> {
        (0..n).map(|i| amp * (2.0 * std::f32::consts::PI * hz * i as f32 / FS).sin()).collect()
    }

    fn analyse(x: &[f32]) -> [f32; SPECTRUM_BANDS] {
        let mut a = SpectrumAnalyzer::new(FS);
        a.push(x);
        assert!(a.ready());
        let mut out = [0.0; SPECTRUM_BANDS];
        a.compute(&mut out);
        out
    }

    #[test]
    fn bands_are_log_spaced_from_20_hz_to_20_khz() {
        assert!((band_frequency(0) - 20.0).abs() < 1e-3);
        assert!((band_frequency(SPECTRUM_BANDS - 1) - 20_000.0).abs() < 1.0);
        assert!(((band_frequency(48) / band_frequency(47)) - (band_frequency(1) / band_frequency(0))).abs() < 1e-3);
    }

    #[test]
    fn a_sine_peaks_in_the_right_band_at_the_right_level() {
        for (hz, amp_db) in [(100.0f32, -6.0f32), (1000.0, -6.0), (5000.0, -20.0), (12_000.0, -12.0)] {
            let out = analyse(&sine(hz, 10f32.powf(amp_db / 20.0), SPECTRUM_FFT * 2));
            let (peak_band, peak) = out.iter().copied().enumerate().fold((0, f32::MIN), |m, (i, v)| if v > m.1 { (i, v) } else { m });
            let fc = band_frequency(peak_band);
            assert!((fc / hz - 1.0).abs() < 0.07, "{hz} Hz peaked at {fc} Hz");
            assert!((peak - amp_db).abs() < 1.6, "{hz} Hz read {peak} dB, expected {amp_db}");
        }
    }

    #[test]
    fn energy_far_from_the_tone_is_far_below_it() {
        let out = analyse(&sine(1000.0, 0.5, SPECTRUM_FFT));
        let b100 = (0..SPECTRUM_BANDS).min_by_key(|&i| ((band_frequency(i) - 100.0).abs() * 100.0) as i64).unwrap();
        let b10k = (0..SPECTRUM_BANDS).min_by_key(|&i| ((band_frequency(i) - 10_000.0).abs() * 100.0) as i64).unwrap();
        assert!(out[b100] < -60.0 && out[b10k] < -60.0, "{} {}", out[b100], out[b10k]);
    }

    #[test]
    fn silence_is_at_the_floor_and_not_ready_before_a_full_window() {
        let out = analyse(&vec![0.0; SPECTRUM_FFT]);
        assert!(out.iter().all(|&v| v == SPECTRUM_FLOOR));
        let mut a = SpectrumAnalyzer::new(FS);
        a.push(&[0.1; 100]);
        assert!(!a.ready());
    }

    #[test]
    fn follows_the_newest_audio_when_more_than_a_window_is_pushed() {
        let mut a = SpectrumAnalyzer::new(FS);
        a.push(&sine(200.0, 0.5, SPECTRUM_FFT * 3));
        a.push(&sine(4000.0, 0.5, SPECTRUM_FFT * 2));
        let mut out = [0.0; SPECTRUM_BANDS];
        a.compute(&mut out);
        let peak_band = out.iter().copied().enumerate().fold((0, f32::MIN), |m, (i, v)| if v > m.1 { (i, v) } else { m }).0;
        assert!((band_frequency(peak_band) / 4000.0 - 1.0).abs() < 0.07);
    }
}
