//! Audiora mixing engine.
//!
//! The control side (UI, IPC, presets) talks to the audio thread only through
//! lock-free atomics in [`MixerParams`]; the audio thread owns all DSP state.

mod atomic;
mod mixer;
mod params;
mod paths;
mod drift;
pub mod ring;

pub use atomic::AtomicF32;
pub use drift::{DriftResampler, FillReport};
pub use mixer::{Mixer, MAX_BLOCK_FRAMES, POST_TAP_FRAMES};
pub use params::{BusParams, EqBandParams, MixerParams, StripParams, EQ_BANDS};
pub use paths::ParamError;
