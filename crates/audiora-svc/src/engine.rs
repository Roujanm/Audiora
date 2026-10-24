//! Live audio graph: capture thread -> ring -> render thread (mixer -> device).
//! Each device stream lives on its own thread because WASAPI/COM objects are
//! thread-affine; the only shared state is atomics and a wait-free ring.

use std::f32::consts::TAU;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc};
use std::thread::JoinHandle;

use audiora_core::{ring, Mixer, MixerParams, MAX_BLOCK_FRAMES};
use audiora_dsp::LinearResampler;
use audiora_hal::{CaptureStream, RenderStream, StreamFormat};

use crate::rec::{self, AudioTap, RecController, TapSlot};

#[derive(Clone, Debug, Default)]
pub struct EngineConfig {
    /// `None` = default capture device when `use_input`, see `input_enabled`.
    pub input: Option<String>,
    pub input_enabled: bool,
    pub output: Option<String>,
    /// Built-in oscillator on strip 1, for tests without a microphone.
    pub tone_hz: Option<f32>,
    /// Where takes are written; `None` = `%USERPROFILE%\Music\Audiora\Recordings`.
    pub rec_dir: Option<std::path::PathBuf>,
}

#[derive(Default)]
pub struct Stats {
    pub callbacks: AtomicU64,
    /// Render callback needed audio the capture side had not delivered yet.
    pub underruns: AtomicU64,
    /// Capture ran ahead of render (clock drift); old audio was discarded.
    pub drift_drops: AtomicU64,
    /// Capture could not enqueue (ring full).
    pub capture_drops: AtomicU64,
}

#[derive(Clone, Copy, Debug)]
pub struct EngineInfo {
    pub output: StreamFormat,
    pub input: Option<StreamFormat>,
}

pub struct Engine {
    pub params: Arc<MixerParams>,
    pub stats: Arc<Stats>,
    pub info: EngineInfo,
    pub rec: Arc<RecController>,
    stop: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
}

pub const STRIP_TONE: usize = 1;

impl Engine {
    pub fn start(cfg: EngineConfig) -> Result<Engine, String> {
        let stop = Arc::new(AtomicBool::new(false));
        let stats = Arc::new(Stats::default());
        let (prod, cons) = ring::ring(1 << 17);
        let mut threads = Vec::new();
        let slot = Arc::new(TapSlot::default());

        let (tx, rx) = mpsc::channel::<Result<(StreamFormat, Arc<MixerParams>), String>>();
        {
            let (stop, stats, cfg, slot) = (stop.clone(), stats.clone(), cfg.clone(), slot.clone());
            threads.push(std::thread::spawn(move || render_thread(cfg, cons, stop, stats, slot, tx)));
        }
        let (out_fmt, params) = rx
            .recv()
            .map_err(|_| "render thread died".to_string())?
            .map_err(|e| format!("output device: {e}"))?;

        let mut in_fmt = None;
        if cfg.input_enabled {
            let (tx, rx) = mpsc::channel::<Result<StreamFormat, String>>();
            let (stop2, stats2, cfg2) = (stop.clone(), stats.clone(), cfg.clone());
            threads.push(std::thread::spawn(move || {
                capture_thread(cfg2, out_fmt.sample_rate, prod, stop2, stats2, tx)
            }));
            match rx.recv().map_err(|_| "capture thread died".to_string())? {
                Ok(f) => in_fmt = Some(f),
                Err(e) => {
                    stop.store(true, Ordering::Relaxed);
                    for t in threads {
                        let _ = t.join();
                    }
                    return Err(format!("input device: {e}"));
                }
            }
        }

        let rec_dir = cfg.rec_dir.clone().unwrap_or_else(rec::default_dir);
        let rec = Arc::new(RecController::new(slot, out_fmt.sample_rate, rec_dir));
        Ok(Engine {
            params,
            stats,
            info: EngineInfo { output: out_fmt, input: in_fmt },
            rec,
            stop,
            threads,
        })
    }

    pub fn stop_flag(&self) -> Arc<AtomicBool> {
        self.stop.clone()
    }

    pub fn stop(self) {
        // Finalize an in-progress take while the audio thread can still detach.
        if let Ok(s) = self.rec.stop() {
            println!("recording saved: {} ({:.1} s, {} dropped frames)", s.path.display(), s.seconds, s.dropped_frames);
        }
        self.stop.store(true, Ordering::Relaxed);
        for t in self.threads {
            let _ = t.join();
        }
    }
}

fn render_thread(
    cfg: EngineConfig,
    mut cons: ring::Consumer,
    stop: Arc<AtomicBool>,
    stats: Arc<Stats>,
    slot: Arc<TapSlot>,
    ready: mpsc::Sender<Result<(StreamFormat, Arc<MixerParams>), String>>,
) {
    let stream = match RenderStream::open(cfg.output.as_deref()) {
        Ok(s) => s,
        Err(e) => {
            let _ = ready.send(Err(e.to_string()));
            return;
        }
    };
    let fmt = stream.format();
    let (mut mixer, params) = Mixer::new(fmt.sample_rate as f32, 2, 1);
    params.strips[STRIP_TONE].gain_db.set(-30.0);
    let _ = ready.send(Ok((fmt, params)));

    let cap = (fmt.buffer_frames as usize).max(MAX_BLOCK_FRAMES) * 2;
    let mut in_mic = vec![0f32; cap];
    let mut in_tone = vec![0f32; cap];
    let mut bus = vec![0f32; cap];
    let ch = fmt.channels as usize;
    let target = fmt.buffer_frames as usize * 2;
    let high_water = target * 4;
    let tone_step = cfg.tone_hz.map(|hz| TAU * hz / fmt.sample_rate as f32);
    let mut phase = 0f32;
    let mut tap = AudioTap::new(slot, cap / 2);

    let result = stream.run_until(&stop, None, |out, frames| {
        let n = frames * 2;
        stats.callbacks.fetch_add(1, Ordering::Relaxed);
        tap.poll();

        // Keep latency bounded: if capture ran ahead, drop the oldest audio.
        let avail = cons.available();
        if avail > high_water {
            let mut excess = avail - target;
            while excess > 0 {
                let got = cons.pop_slice(&mut bus[..excess.min(cap)]);
                if got == 0 {
                    break;
                }
                excess -= got;
            }
            stats.drift_drops.fetch_add(1, Ordering::Relaxed);
        }
        let got = cons.pop_slice(&mut in_mic[..n]);
        if got < n {
            in_mic[got..n].fill(0.0);
            if cfg.input_enabled {
                stats.underruns.fetch_add(1, Ordering::Relaxed);
            }
        }

        match tone_step {
            Some(step) => {
                for f in in_tone[..n].chunks_exact_mut(2) {
                    let s = phase.sin();
                    f[0] = s;
                    f[1] = s;
                    phase = (phase + step) % TAU;
                }
            }
            None => in_tone[..n].fill(0.0),
        }

        mixer.process(&[&in_mic[..n], &in_tone[..n]], &mut [&mut bus[..n]]);
        tap.push(frames, [&in_mic[..n], &in_tone[..n], &bus[..n]]);
        for (i, frame) in out.chunks_exact_mut(ch).enumerate() {
            frame[0] = bus[i * 2];
            if ch > 1 {
                frame[1] = bus[i * 2 + 1];
            }
        }
    });
    if let Err(e) = result {
        eprintln!("render stopped: {e}");
        stop.store(true, Ordering::Relaxed);
    }
}

fn capture_thread(
    cfg: EngineConfig,
    out_rate: u32,
    mut prod: ring::Producer,
    stop: Arc<AtomicBool>,
    stats: Arc<Stats>,
    ready: mpsc::Sender<Result<StreamFormat, String>>,
) {
    let stream = match CaptureStream::open(cfg.input.as_deref()) {
        Ok(s) => s,
        Err(e) => {
            let _ = ready.send(Err(e.to_string()));
            return;
        }
    };
    let fmt = stream.format();
    let _ = ready.send(Ok(fmt));

    let ch = fmt.channels as usize;
    let mut resampler = LinearResampler::new(fmt.sample_rate, out_rate);
    let max_frames = fmt.buffer_frames as usize * 2;
    let mut stereo = vec![0f32; max_frames * 2];
    let mut resampled = vec![0f32; resampler.max_output_frames(max_frames) * 2];

    let result = stream.run_until(&stop, |data, frames| {
        if frames > max_frames {
            return;
        }
        for (i, f) in data.chunks_exact(ch).enumerate().take(frames) {
            stereo[i * 2] = f[0];
            stereo[i * 2 + 1] = if ch > 1 { f[1] } else { f[0] };
        }
        let block = &stereo[..frames * 2];
        let pushed = if fmt.sample_rate == out_rate {
            let n = prod.push_slice(block);
            n == block.len()
        } else {
            let n = resampler.process(block, &mut resampled);
            prod.push_slice(&resampled[..n * 2]) == n * 2
        };
        if !pushed {
            stats.capture_drops.fetch_add(1, Ordering::Relaxed);
        }
    });
    if let Err(e) = result {
        eprintln!("capture stopped: {e}");
        stop.store(true, Ordering::Relaxed);
    }
}
