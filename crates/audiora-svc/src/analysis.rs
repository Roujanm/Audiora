//! Metering that is too heavy for the audio callback: programme loudness (LUFS) of the master
//! bus and a spectrum of one selectable signal. The audio thread only copies blocks into
//! wait-free rings; this module's thread does the FFT and the K-weighted filtering.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use audiora_core::ring::{self, Consumer, Producer};
use audiora_core::AtomicF32;
use audiora_dsp::{LoudnessMeter, SpectrumAnalyzer, LUFS_FLOOR, SPECTRUM_BANDS, SPECTRUM_FLOOR};

/// `source` value meaning "spectrum analyser off".
pub const SPECTRUM_OFF: usize = usize::MAX;

const SPECTRUM_HOP: usize = 1024;
const POLL: Duration = Duration::from_millis(8);

/// Results shared with the control threads.
pub struct Analysis {
    /// Index into the engine's source list (strips, then buses) feeding the spectrum.
    pub source: AtomicUsize,
    pub momentary: AtomicF32,
    pub short_term: AtomicF32,
    pub integrated: AtomicF32,
    pub reset: AtomicBool,
    spectrum: Mutex<[f32; SPECTRUM_BANDS]>,
}

impl Default for Analysis {
    fn default() -> Self {
        Self {
            source: AtomicUsize::new(SPECTRUM_OFF),
            momentary: AtomicF32::new(LUFS_FLOOR),
            short_term: AtomicF32::new(LUFS_FLOOR),
            integrated: AtomicF32::new(LUFS_FLOOR),
            reset: AtomicBool::new(false),
            spectrum: Mutex::new([SPECTRUM_FLOOR; SPECTRUM_BANDS]),
        }
    }
}

impl Analysis {
    pub fn spectrum(&self) -> Option<[f32; SPECTRUM_BANDS]> {
        (self.source.load(Ordering::Relaxed) != SPECTRUM_OFF).then(|| *self.spectrum.lock().unwrap())
    }
}

/// Audio-thread half: copies blocks into rings, never blocks or allocates.
pub struct AnalysisTap {
    shared: Arc<Analysis>,
    spectrum_prod: Producer,
    master_prod: Producer,
    mono: Vec<f32>,
}

impl AnalysisTap {
    /// `sources` are interleaved stereo blocks (strips then buses); `master` is bus 0.
    pub fn push(&mut self, sources: &[&[f32]], master: &[f32]) {
        let src = self.shared.source.load(Ordering::Relaxed);
        if let Some(block) = sources.get(src) {
            let frames = (block.len() / 2).min(self.mono.len());
            for (m, f) in self.mono[..frames].iter_mut().zip(block.chunks_exact(2)) {
                *m = 0.5 * (f[0] + f[1]);
            }
            if self.spectrum_prod.free() >= frames {
                self.spectrum_prod.push_slice(&self.mono[..frames]);
            }
        }
        if self.master_prod.free() >= master.len() {
            self.master_prod.push_slice(master);
        }
    }
}

/// Starts the analysis thread; returns the audio-side tap and the thread handle.
pub fn spawn(
    shared: Arc<Analysis>,
    sample_rate: f32,
    max_block_frames: usize,
    stop: Arc<AtomicBool>,
) -> (AnalysisTap, JoinHandle<()>) {
    let (spectrum_prod, spectrum_cons) = ring::ring(1 << 16);
    let (master_prod, master_cons) = ring::ring(1 << 17);
    let tap = AnalysisTap { shared: shared.clone(), spectrum_prod, master_prod, mono: vec![0.0; max_block_frames] };
    let handle = std::thread::Builder::new()
        .name("audiora-analysis".into())
        .spawn(move || run(shared, sample_rate, spectrum_cons, master_cons, stop))
        .expect("spawn analysis thread");
    (tap, handle)
}

fn run(shared: Arc<Analysis>, fs: f32, mut spectrum_cons: Consumer, mut master_cons: Consumer, stop: Arc<AtomicBool>) {
    let mut loudness = LoudnessMeter::new(fs);
    let mut analyzer = SpectrumAnalyzer::new(fs);
    let mut buf = vec![0f32; 8192];
    let mut since_compute = 0usize;
    let mut bands = [SPECTRUM_FLOOR; SPECTRUM_BANDS];
    let mut last_source = SPECTRUM_OFF;

    while !stop.load(Ordering::Relaxed) {
        std::thread::sleep(POLL);

        if shared.reset.swap(false, Ordering::Relaxed) {
            loudness.reset();
        }
        loop {
            let n = master_cons.pop_slice(&mut buf[..8190]);
            if n == 0 {
                break;
            }
            loudness.process(&buf[..n & !1]);
        }
        shared.momentary.set(loudness.momentary());
        shared.short_term.set(loudness.short_term());
        shared.integrated.set(loudness.integrated());

        let source = shared.source.load(Ordering::Relaxed);
        if source != last_source {
            analyzer = SpectrumAnalyzer::new(fs);
            since_compute = 0;
            last_source = source;
        }
        loop {
            let n = spectrum_cons.pop_slice(&mut buf);
            if n == 0 {
                break;
            }
            if source == SPECTRUM_OFF {
                continue; // discard while off
            }
            analyzer.push(&buf[..n]);
            since_compute += n;
            while since_compute >= SPECTRUM_HOP && analyzer.ready() {
                since_compute -= SPECTRUM_HOP;
                if since_compute < SPECTRUM_HOP {
                    analyzer.compute(&mut bands);
                    *shared.spectrum.lock().unwrap() = bands;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use audiora_dsp::band_frequency;

    fn tone_block(frames: usize, start: usize, hz: f32, amp: f32) -> Vec<f32> {
        (start..start + frames)
            .flat_map(|n| {
                let s = amp * (2.0 * std::f32::consts::PI * hz * n as f32 / 48_000.0).sin();
                [s, s]
            })
            .collect()
    }

    #[test]
    fn thread_measures_loudness_and_selected_spectrum() {
        let shared = Arc::new(Analysis::default());
        let stop = Arc::new(AtomicBool::new(false));
        let (mut tap, handle) = spawn(shared.clone(), 48_000.0, 4096, stop.clone());
        shared.source.store(1, Ordering::Relaxed); // second source

        // feed 4 s of 1 kHz at -23 dBFS to the master and to source 1 (source 0 is silent)
        let amp = 10f32.powf(-23.0 / 20.0);
        let silent = vec![0.0f32; 960 * 2];
        let mut pos = 0;
        for _ in 0..200 {
            let block = tone_block(960, pos, 1000.0, amp);
            tap.push(&[&silent, &block], &block);
            pos += 960;
            std::thread::sleep(Duration::from_millis(20));
        }
        std::thread::sleep(Duration::from_millis(150));

        let m = shared.momentary.get();
        assert!((m + 23.0).abs() < 0.3, "momentary {m}");
        assert!((shared.integrated.get() + 23.0).abs() < 0.3, "integrated {}", shared.integrated.get());
        let spec = shared.spectrum().expect("source selected");
        let peak = spec.iter().copied().enumerate().fold((0, f32::MIN), |a, (i, v)| if v > a.1 { (i, v) } else { a });
        assert!((band_frequency(peak.0) / 1000.0 - 1.0).abs() < 0.07, "peak at {} Hz", band_frequency(peak.0));
        assert!((peak.1 + 23.0).abs() < 1.8, "level {}", peak.1);

        shared.reset.store(true, Ordering::Relaxed);
        shared.source.store(SPECTRUM_OFF, Ordering::Relaxed);
        std::thread::sleep(Duration::from_millis(80));
        assert_eq!(shared.integrated.get(), LUFS_FLOOR, "reset clears the gated measurement");
        assert!(shared.spectrum().is_none(), "off means no spectrum");

        stop.store(true, Ordering::Relaxed);
        handle.join().unwrap();
    }
}
