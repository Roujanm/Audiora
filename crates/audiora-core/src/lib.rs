//! Audiora mixing engine.
//!
//! The control side (UI, IPC, presets) talks to the audio thread only through
//! lock-free atomics in [`MixerParams`]; the audio thread owns all DSP state.

mod atomic;
mod mixer;
mod params;
mod paths;
pub mod ring;

pub use atomic::AtomicF32;
pub use mixer::{Mixer, MAX_BLOCK_FRAMES};
pub use params::{BusParams, EqBandParams, MixerParams, StripParams, EQ_BANDS};
pub use paths::ParamError;
