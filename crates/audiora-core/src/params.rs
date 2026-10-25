use std::sync::atomic::{AtomicBool, AtomicU32};

use audiora_dsp::{CompSettings, GateSettings, LimiterSettings};

use crate::AtomicF32;

pub const EQ_BANDS: usize = 6;

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

#[derive(Debug)]
pub struct GateParams {
    pub enabled: AtomicBool,
    pub threshold_db: AtomicF32,
    pub attack_ms: AtomicF32,
    pub release_ms: AtomicF32,
    pub range_db: AtomicF32,
    /// Meter: 1.0 while open.
    pub open: AtomicF32,
}

impl GateParams {
    fn new() -> Self {
        let d = GateSettings::default();
        Self {
            enabled: AtomicBool::new(false),
            threshold_db: AtomicF32::new(d.threshold_db),
            attack_ms: AtomicF32::new(d.attack_ms),
            release_ms: AtomicF32::new(d.release_ms),
            range_db: AtomicF32::new(d.range_db),
            open: AtomicF32::new(1.0),
        }
    }

    pub fn settings(&self) -> GateSettings {
        GateSettings {
            threshold_db: self.threshold_db.get(),
            attack_ms: self.attack_ms.get(),
            release_ms: self.release_ms.get(),
            range_db: self.range_db.get(),
        }
    }
}

#[derive(Debug)]
pub struct CompParams {
    pub enabled: AtomicBool,
    pub threshold_db: AtomicF32,
    pub ratio: AtomicF32,
    pub attack_ms: AtomicF32,
    pub release_ms: AtomicF32,
    pub makeup_db: AtomicF32,
    pub knee_db: AtomicF32,
    /// Meter: current gain reduction in dB (>= 0).
    pub gr_db: AtomicF32,
}

impl CompParams {
    fn new() -> Self {
        let d = CompSettings::default();
        Self {
            enabled: AtomicBool::new(false),
            threshold_db: AtomicF32::new(d.threshold_db),
            ratio: AtomicF32::new(d.ratio),
            attack_ms: AtomicF32::new(d.attack_ms),
            release_ms: AtomicF32::new(d.release_ms),
            makeup_db: AtomicF32::new(d.makeup_db),
            knee_db: AtomicF32::new(d.knee_db),
            gr_db: AtomicF32::new(0.0),
        }
    }

    pub fn settings(&self) -> CompSettings {
        CompSettings {
            threshold_db: self.threshold_db.get(),
            ratio: self.ratio.get(),
            attack_ms: self.attack_ms.get(),
            release_ms: self.release_ms.get(),
            makeup_db: self.makeup_db.get(),
            knee_db: self.knee_db.get(),
        }
    }
}

/// Smart Volume: automatic leveling towards a target loudness.
#[derive(Debug)]
pub struct SmartParams {
    pub enabled: AtomicBool,
    /// Target loudness in dBFS (roughly RMS).
    pub target_db: AtomicF32,
    /// Meter: gain currently applied in dB.
    pub gain_db: AtomicF32,
}

impl SmartParams {
    fn new() -> Self {
        Self { enabled: AtomicBool::new(false), target_db: AtomicF32::new(-24.0), gain_db: AtomicF32::new(0.0) }
    }
}

#[derive(Debug)]
pub struct LimiterParams {
    pub enabled: AtomicBool,
    pub ceiling_db: AtomicF32,
    pub release_ms: AtomicF32,
    pub gr_db: AtomicF32,
}

impl LimiterParams {
    fn new() -> Self {
        let d = LimiterSettings::default();
        Self {
            enabled: AtomicBool::new(false),
            ceiling_db: AtomicF32::new(d.ceiling_db),
            release_ms: AtomicF32::new(d.release_ms),
            gr_db: AtomicF32::new(0.0),
        }
    }

    pub fn settings(&self) -> LimiterSettings {
        LimiterSettings { ceiling_db: self.ceiling_db.get(), release_ms: self.release_ms.get() }
    }
}

/// Control-thread view of one input strip. Band layout is fixed:
/// 0 = low shelf (sub bass), 1-4 = peaks (bass, low mids, mids, upper mids), 5 = high shelf.
/// Signal order: boost -> denoise -> gate -> EQ -> smart volume -> compressor -> fader/balance.
#[derive(Debug)]
pub struct StripParams {
    pub gain_db: AtomicF32,
    /// -1.0 (left) .. 1.0 (right); 0.0 is unity on both channels.
    pub balance: AtomicF32,
    /// Extra gain ahead of the processing chain, 0..=12 dB ("volume boost").
    pub boost_db: AtomicF32,
    pub mute: AtomicBool,
    pub solo: AtomicBool,
    /// Bit `n` set = strip feeds bus `n`.
    pub bus_mask: AtomicU32,
    pub denoise: AtomicBool,
    pub gate: GateParams,
    pub eq: [EqBandParams; EQ_BANDS],
    pub smart: SmartParams,
    pub comp: CompParams,
    /// Post-fader peak of the last processed call (linear).
    pub peak_l: AtomicF32,
    pub peak_r: AtomicF32,
}

impl StripParams {
    fn new() -> Self {
        Self {
            gain_db: AtomicF32::new(0.0),
            balance: AtomicF32::new(0.0),
            boost_db: AtomicF32::new(0.0),
            mute: AtomicBool::new(false),
            solo: AtomicBool::new(false),
            bus_mask: AtomicU32::new(1),
            denoise: AtomicBool::new(false),
            gate: GateParams::new(),
            eq: [
                EqBandParams::new(60.0, 0.707),
                EqBandParams::new(160.0, 1.0),
                EqBandParams::new(500.0, 1.0),
                EqBandParams::new(1800.0, 1.0),
                EqBandParams::new(5000.0, 1.0),
                EqBandParams::new(10000.0, 0.707),
            ],
            smart: SmartParams::new(),
            comp: CompParams::new(),
            peak_l: AtomicF32::new(0.0),
            peak_r: AtomicF32::new(0.0),
        }
    }
}

#[derive(Debug)]
pub struct BusParams {
    pub gain_db: AtomicF32,
    pub mute: AtomicBool,
    /// Brick-wall limiter after the bus fader.
    pub limiter: LimiterParams,
    pub peak_l: AtomicF32,
    pub peak_r: AtomicF32,
}

impl BusParams {
    fn new() -> Self {
        Self {
            gain_db: AtomicF32::new(0.0),
            mute: AtomicBool::new(false),
            limiter: LimiterParams::new(),
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
