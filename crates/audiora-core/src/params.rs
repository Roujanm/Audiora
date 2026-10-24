use std::sync::atomic::{AtomicBool, AtomicU32};

use crate::AtomicF32;

pub const EQ_BANDS: usize = 4;

#[derive(Debug)]
pub struct EqBandParams {
    pub freq: AtomicF32,
    pub gain_db: AtomicF32,
    pub q: AtomicF32,
}

impl EqBandParams {
    fn new(freq: f32, q: f32) -> Self {
        Self { freq: AtomicF32::new(freq), gain_db: AtomicF32::new(0.0), q: AtomicF32::new(q) }
    }
}

/// Control-thread view of one input strip. Band layout is fixed:
/// 0 = low shelf, 1 = peak, 2 = peak, 3 = high shelf.
#[derive(Debug)]
pub struct StripParams {
    pub gain_db: AtomicF32,
    /// -1.0 (left) .. 1.0 (right); 0.0 is unity on both channels.
    pub balance: AtomicF32,
    pub mute: AtomicBool,
    pub solo: AtomicBool,
    /// Bit `n` set = strip feeds bus `n`.
    pub bus_mask: AtomicU32,
    pub eq: [EqBandParams; EQ_BANDS],
    /// Post-fader peak of the last processed call (linear).
    pub peak_l: AtomicF32,
    pub peak_r: AtomicF32,
}

impl StripParams {
    fn new() -> Self {
        Self {
            gain_db: AtomicF32::new(0.0),
            balance: AtomicF32::new(0.0),
            mute: AtomicBool::new(false),
            solo: AtomicBool::new(false),
            bus_mask: AtomicU32::new(1),
            eq: [
                EqBandParams::new(100.0, 0.707),
                EqBandParams::new(400.0, 1.0),
                EqBandParams::new(2500.0, 1.0),
                EqBandParams::new(8000.0, 0.707),
            ],
            peak_l: AtomicF32::new(0.0),
            peak_r: AtomicF32::new(0.0),
        }
    }
}

#[derive(Debug)]
pub struct BusParams {
    pub gain_db: AtomicF32,
    pub mute: AtomicBool,
    pub peak_l: AtomicF32,
    pub peak_r: AtomicF32,
}

impl BusParams {
    fn new() -> Self {
        Self {
            gain_db: AtomicF32::new(0.0),
            mute: AtomicBool::new(false),
            peak_l: AtomicF32::new(0.0),
            peak_r: AtomicF32::new(0.0),
        }
    }
}

#[derive(Debug)]
pub struct MixerParams {
    pub strips: Vec<StripParams>,
    pub buses: Vec<BusParams>,
}

impl MixerParams {
    pub fn new(strips: usize, buses: usize) -> Self {
        assert!(buses <= 32, "bus_mask is 32 bits wide");
        Self {
            strips: (0..strips).map(|_| StripParams::new()).collect(),
            buses: (0..buses).map(|_| BusParams::new()).collect(),
        }
    }
}
