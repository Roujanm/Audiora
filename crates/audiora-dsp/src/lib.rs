//! Real-time safe DSP primitives. Nothing in this crate allocates or locks
//! after construction, so every type is safe to use on the audio thread.

mod biquad;
mod denoise;
mod dynamics;
mod loudness;
mod spectrum;
mod resample;

pub use biquad::{Biquad, BiquadCoeffs, FilterKind};
pub use denoise::{Denoiser, DENOISE_FRAME, DENOISE_SAMPLE_RATE};
pub use dynamics::{CompSettings, Compressor, Gate, GateSettings, Limiter, LimiterSettings};
pub use loudness::{LoudnessMeter, LUFS_FLOOR};
pub use spectrum::{band_frequency, SpectrumAnalyzer, SPECTRUM_BANDS, SPECTRUM_FFT, SPECTRUM_FLOOR};
pub use resample::LinearResampler;

/// Silence floor used when converting linear gain to dB.
pub const MIN_DB: f32 = -120.0;

#[inline]
pub fn db_to_lin(db: f32) -> f32 {
    if db <= MIN_DB {
        0.0
    } else {
        10f32.powf(db / 20.0)
    }
}

#[inline]
pub fn lin_to_db(lin: f32) -> f32 {
    if lin <= 1e-6 {
        MIN_DB
    } else {
        20.0 * lin.log10()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db_roundtrip() {
        for db in [-60.0, -6.0, 0.0, 6.0, 12.0] {
            assert!((lin_to_db(db_to_lin(db)) - db).abs() < 1e-3);
        }
        assert_eq!(db_to_lin(-200.0), 0.0);
        assert!((db_to_lin(-6.0206) - 0.5).abs() < 1e-3);
    }
}
