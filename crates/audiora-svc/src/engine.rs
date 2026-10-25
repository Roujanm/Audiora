//! Live audio graph.
//!
//! ```text
//!  capture dev 0 ─ring─ DriftResampler ─┐
//!  capture dev N ─ring─ DriftResampler ─┼─► Mixer (strips → buses) ─► master device (bus 0)
//!  tone generator ──────────────────────┘                    └─ring─► DriftResampler ─► device for bus 1..
//! ```
//!
//! Every device stream lives on its own thread because WASAPI/COM objects are
//! thread-affine, and every device is its own clock domain: audio crosses
//! between them only through wait-free rings read by a [`DriftResampler`].
//! The master device (bus 0) drives the mixer.

use std::f32::consts::TAU;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::JoinHandle;

use arrayvec::ArrayVec;
use audiora_core::{ring, DriftResampler, Mixer, MixerParams, MAX_BLOCK_FRAMES};
use audiora_hal::{list_devices, CaptureStream, Flow, Mode, ProcessCapture, RenderStream, StreamFormat};

use crate::analysis::{self, Analysis};
use crate::rec::{self, AudioTap, RecController, TapSlot};

pub const MAX_STRIPS: usize = 16;
pub const MAX_BUSES: usize = 8;
const RING_SAMPLES: usize = 1 << 17;

/// One mixer strip fed from outside the graph.
#[derive(Clone, Debug, Default)]
pub struct InputSpec {
    /// Device id (`None` = system default).
    pub device: Option<String>,
    /// Tap what a render device is playing instead of recording a capture device. This is how an
    /// application mix reaches the mixer: apps are routed to a virtual output, Audiora listens to it.
    pub loopback: bool,
    /// No device at all: the strip stays in the layout but is silent.
    pub silent: bool,
    /// Display name overriding the device name.
    pub name: Option<String>,
    /// Fixed channel this strip belongs to ("game", "chat", "media", "aux", "mic").
    pub channel: Option<&'static str>,
    /// Why a silent strip is silent, shown to the user.
    pub note: Option<String>,
    /// Capture the microphone without Windows/driver effects.
    pub raw: bool,
}

impl InputSpec {
    pub fn capture(device: Option<String>) -> Self {
        Self { device, ..Self::default() }
    }
}

#[derive(Clone, Default)]
pub struct EngineConfig {
    /// One entry per input strip. Empty = no inputs.
    pub inputs: Vec<InputSpec>,
    /// An input that cannot be opened leaves its strip silent (and reports why) instead of
    /// stopping the engine.
    pub tolerant_inputs: bool,
    /// One entry per output bus: the first is the master device that clocks
    /// the mixer. Must not be empty.
    pub outputs: Vec<Option<String>>,
    /// Built-in oscillator strip (always present, silent unless set).
    pub tone_hz: Option<f32>,
    /// Where takes are written; `None` = `%USERPROFILE%\Music\Audiora\Recordings`.
    pub rec_dir: Option<PathBuf>,
    /// How device streams are opened: shared, low-latency shared, or exclusive.
    pub mode: Mode,
    /// Strip recorded by the microphone check.
    pub mic_strip: Option<usize>,
    /// When the master device disappears for a few seconds, asks for another output to play on
    /// (for example the new system default). `None` waits for the same device to return.
    pub master_fallback: Option<MasterFallback>,
}

pub type MasterFallback = Arc<dyn Fn() -> Option<String> + Send + Sync>;

#[derive(Default)]
pub struct Stats {
    pub callbacks: AtomicU64,
    /// A device read from a ring that had run dry.
    pub underruns: AtomicU64,
    /// A ring overfilled (clock drift too large to trim) and old audio was discarded.
    pub drift_drops: AtomicU64,
    /// A producer could not enqueue (ring full).
    pub capture_drops: AtomicU64,
    /// A device stream ended unexpectedly (unplugged, taken exclusively, driver reset).
    pub device_errors: AtomicU64,
}

#[derive(Clone, Debug)]
pub struct NodeInfo {
    /// "input" or "tone" for strips; "bus" for buses.
    pub kind: &'static str,
    pub name: String,
    pub device_id: Option<String>,
    pub format: Option<StreamFormat>,
    /// Fixed channel key for channel-mode strips.
    pub channel: Option<&'static str>,
    /// Why the strip is silent although it was configured.
    pub error: Option<String>,
}

#[derive(Clone, Debug)]
pub struct EngineInfo {
    pub strips: Vec<NodeInfo>,
    pub buses: Vec<NodeInfo>,
}

pub struct Engine {
    pub params: Arc<MixerParams>,
    pub stats: Arc<Stats>,
    pub info: EngineInfo,
    pub rec: Arc<RecController>,
    pub analysis: Arc<Analysis>,
    /// Adds and removes per-application captures while the engine runs.
    pub tapper: AppTapper,
    pub mic_test: Arc<MicTest>,
    pub playback: Arc<Playback>,
    stop: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
}

/// Longest microphone check, in seconds.
pub const MIC_TEST_MAX_SECS: usize = 10;

/// "Record yourself, then listen back": the processed microphone (exactly what others receive) is
/// recorded into memory and can be played into the master. Driven from the control thread through
/// these atomics; the audio thread owns the buffer.
#[derive(Default)]
pub struct MicTest {
    /// 1 = record, 2 = play, 3 = stop; consumed by the audio thread.
    cmd: AtomicU8,
    /// 0 = idle, 1 = recording, 2 = playing.
    pub state: AtomicU8,
    /// Frames recorded or played so far.
    pub pos: AtomicUsize,
    /// Frames in the last recording.
    pub len: AtomicUsize,
    /// Frames to record before stopping by itself.
    limit: AtomicUsize,
    /// Strip that is recorded (the microphone).
    pub strip: AtomicUsize,
    pub rate: AtomicUsize,
}

impl MicTest {
    pub fn record(&self, seconds: f64) {
        let rate = self.rate.load(Ordering::Relaxed).max(1);
        let secs = seconds.clamp(1.0, MIC_TEST_MAX_SECS as f64);
        self.limit.store((secs * rate as f64) as usize, Ordering::Relaxed);
        self.cmd.store(1, Ordering::Release);
    }

    pub fn play(&self) {
        self.cmd.store(2, Ordering::Release);
    }

    pub fn stop(&self) {
        self.cmd.store(3, Ordering::Release);
    }
}

/// Studio playback: a producer (the Studio player thread) fills a stereo ring at the master rate,
/// the audio thread adds it to the master after the mixer.
pub struct Playback {
    /// Taken by whoever plays; returned when done.
    pub producer: Mutex<Option<ring::Producer>>,
    /// Set to drop everything queued (seek, stop); the audio thread clears it.
    pub flush: AtomicBool,
    /// Frames the audio thread has played so far (the playhead advances with this).
    pub frames: AtomicU64,
    pub sample_rate: AtomicU64,
}

/// Most applications captured at once; more are refused rather than allocating on the audio thread.
pub const MAX_APP_TAPS: usize = 64;

/// One application's own audio (process loopback, children included), summed into a channel strip.
/// This is how a channel works without any virtual device: the application plays into an output
/// nobody listens to, and Audiora takes its sound straight from Windows.
pub struct AppTap {
    strip: usize,
    cons: ring::Consumer,
    rs: DriftResampler,
    stop: Arc<AtomicBool>,
}

/// Starts application captures; cheap to clone and safe to use from any thread.
#[derive(Clone)]
pub struct AppTapper {
    tx: mpsc::SyncSender<AppTap>,
    master: StreamFormat,
    stats: Arc<Stats>,
    open: Arc<Mutex<Vec<Arc<AtomicBool>>>>,
}

impl AppTapper {
    /// Captures `pid` and its child processes into strip `strip` until the returned flag is set
    /// (or the process goes away, which sets it too).
    pub fn attach(&self, pid: u32, strip: usize) -> Result<Arc<AtomicBool>, String> {
        let (mut prod, cons) = ring::ring(RING_SAMPLES);
        let stop = Arc::new(AtomicBool::new(false));
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
        let (stop2, stats) = (stop.clone(), self.stats.clone());
        std::thread::spawn(move || {
            let cap = match ProcessCapture::open(pid) {
                Ok(c) => c,
                Err(e) => {
                    let _ = ready_tx.send(Err(e.to_string()));
                    return;
                }
            };
            let _ = ready_tx.send(Ok(()));
            let _ = cap.run_until(&stop2, |data, frames| {
                let block = &data[..frames * 2];
                if prod.free() < block.len() {
                    stats.capture_drops.fetch_add(1, Ordering::Relaxed);
                } else {
                    prod.push_slice(block);
                }
            });
            // the process ended or the capture failed: let the mixer drop this tap
            stop2.store(true, Ordering::Relaxed);
        });
        match ready_rx.recv_timeout(std::time::Duration::from_secs(5)) {
            Ok(Ok(())) => {}
            Ok(Err(e)) => return Err(e),
            Err(_) => {
                stop.store(true, Ordering::Relaxed);
                return Err("application capture did not start".into());
            }
        }
        let rate = audiora_hal::proc_loopback_rate();
        let prod_fmt = StreamFormat {
            sample_rate: rate,
            channels: 2,
            buffer_frames: rate / 50,
            period_frames: rate / 100,
            low_latency: false,
            exclusive: false,
            bit_depth: 32,
        };
        let rs = DriftResampler::new(rate, self.master.sample_rate, target_frames(&prod_fmt, &self.master));
        if self.tx.try_send(AppTap { strip, cons, rs, stop: stop.clone() }).is_err() {
            stop.store(true, Ordering::Relaxed);
            return Err(format!("at most {MAX_APP_TAPS} applications can be captured at once"));
        }
        let mut open = self.open.lock().map_err(|_| "tap list poisoned")?;
        open.retain(|f| !f.load(Ordering::Relaxed));
        open.push(stop.clone());
        Ok(stop)
    }

    fn stop_all(&self) {
        if let Ok(open) = self.open.lock() {
            for f in open.iter() {
                f.store(true, Ordering::Relaxed);
            }
        }
    }
}

/// Maps a configured device (or the default) to its friendly name and id.
fn resolver() -> impl Fn(Flow, &Option<String>) -> (String, Option<String>) {
    let all = list_devices().unwrap_or_default();
    move |flow, id| {
        let id = id.clone().or_else(|| audiora_hal::default_device_id(flow).ok());
        let name = id
            .as_ref()
            .and_then(|i| all.iter().find(|d| &d.id == i && d.flow == flow))
            .map_or_else(|| "(unknown device)".to_string(), |d| d.name.clone());
        (name, id)
    }
}

impl Engine {
    pub fn start(cfg: EngineConfig) -> Result<Engine, String> {
        if cfg.outputs.is_empty() || cfg.outputs.len() > MAX_BUSES {
            return Err(format!("need 1..={MAX_BUSES} output devices"));
        }
        if cfg.inputs.len() + 1 > MAX_STRIPS {
            return Err(format!("at most {} input devices", MAX_STRIPS - 1));
        }
        let n_in = cfg.inputs.len();
        let n_out = cfg.outputs.len();
        let resolve = resolver();

        let stop = Arc::new(AtomicBool::new(false));
        let stats = Arc::new(Stats::default());
        let slot = Arc::new(TapSlot::default());
        let analysis = Arc::new(Analysis::default());
        let mut threads: Vec<JoinHandle<()>> = Vec::new();
        let abort = |stop: &Arc<AtomicBool>, threads: Vec<JoinHandle<()>>, msg: String| {
            stop.store(true, Ordering::Relaxed);
            for t in threads {
                let _ = t.join();
            }
            Err::<Engine, String>(msg)
        };

        // Extra output buses first: they open their device, then wait for the master rate.
        let mut extra_prods = Vec::new();
        let mut rate_txs = Vec::new();
        let mut out_formats: Vec<Option<StreamFormat>> = vec![None; n_out];
        for b in 1..n_out {
            let (prod, cons) = ring::ring(RING_SAMPLES);
            extra_prods.push(prod);
            let (rate_tx, rate_rx) = mpsc::channel::<StreamFormat>();
            rate_txs.push(rate_tx);
            let (tx, rx) = mpsc::channel::<Result<StreamFormat, String>>();
            let (dev, stop2, stats2, ll) = (cfg.outputs[b].clone(), stop.clone(), stats.clone(), cfg.mode);
            threads.push(std::thread::spawn(move || output_thread(dev, ll, cons, rate_rx, stop2, stats2, tx)));
            match rx.recv() {
                Ok(Ok(f)) => out_formats[b] = Some(f),
                Ok(Err(e)) => return abort(&stop, threads, format!("output {b}: {e}")),
                Err(_) => return abort(&stop, threads, format!("output {b}: thread died")),
            }
        }

        // Input devices. Silent or failed inputs keep their strip but have no thread.
        let mut in_cons: Vec<Option<ring::Consumer>> = Vec::new();
        let mut in_formats: Vec<Option<StreamFormat>> = Vec::new();
        let mut in_errors: Vec<Option<String>> = Vec::new();
        for (i, spec) in cfg.inputs.iter().enumerate() {
            if spec.silent {
                in_cons.push(None);
                in_formats.push(None);
                in_errors.push(spec.note.clone());
                continue;
            }
            let (prod, cons) = ring::ring(RING_SAMPLES);
            let (tx, rx) = mpsc::channel::<Result<StreamFormat, String>>();
            let (dev, lb, raw, stop2, stats2, ll) = (spec.device.clone(), spec.loopback, spec.raw, stop.clone(), stats.clone(), cfg.mode);
            threads.push(std::thread::spawn(move || capture_thread(dev, lb, raw, ll, prod, stop2, stats2, tx)));
            let opened = match rx.recv() {
                Ok(r) => r,
                Err(_) => Err("thread died".to_string()),
            };
            match opened {
                Ok(f) => {
                    in_formats.push(Some(f));
                    in_cons.push(Some(cons));
                    in_errors.push(None);
                }
                Err(e) if cfg.tolerant_inputs => {
                    eprintln!("input {i} unavailable: {e}");
                    in_formats.push(None);
                    in_cons.push(None);
                    in_errors.push(Some(e));
                }
                Err(e) => return abort(&stop, threads, format!("input {i}: {e}")),
            }
        }

        // Application taps reach the audio thread through a bounded channel (no allocation there).
        let (tap_tx, tap_rx) = mpsc::sync_channel::<AppTap>(MAX_APP_TAPS);
        let mic_test = Arc::new(MicTest::default());
        let (play_prod, play_cons) = ring::ring(RING_SAMPLES * 2);
        let playback = Arc::new(Playback {
            producer: Mutex::new(Some(play_prod)),
            flush: AtomicBool::new(false),
            frames: AtomicU64::new(0),
            sample_rate: AtomicU64::new(0),
        });
        mic_test.strip.store(cfg.mic_strip.unwrap_or(usize::MAX), Ordering::Relaxed);

        // Master render thread: opens the master device and runs the mixer.
        let (tx, rx) = mpsc::channel::<Result<(StreamFormat, Arc<MixerParams>), String>>();
        {
            let args = MasterArgs {
                device: cfg.outputs[0].clone(),
                fallback: cfg.master_fallback.clone(),
                mode: cfg.mode,
                tone_hz: cfg.tone_hz,
                in_cons,
                in_formats: in_formats.clone(),
                in_loopback: cfg.inputs.iter().map(|i| i.loopback).collect(),
                extra_prods,
                rate_txs,
                n_buses: n_out,
                analysis: analysis.clone(),
                taps: tap_rx,
                mic_test: mic_test.clone(),
                playback: playback.clone(),
                play_cons,
            };
            let (stop2, stats2, slot2) = (stop.clone(), stats.clone(), slot.clone());
            threads.push(std::thread::spawn(move || master_thread(args, stop2, stats2, slot2, tx)));
        }
        let (master_fmt, params) = match rx.recv() {
            Ok(Ok(v)) => v,
            Ok(Err(e)) => return abort(&stop, threads, format!("output 0: {e}")),
            Err(_) => return abort(&stop, threads, "output 0: thread died".into()),
        };
        out_formats[0] = Some(master_fmt);

        // Layout shown to clients.
        let mut strips = Vec::new();
        for (i, spec) in cfg.inputs.iter().enumerate() {
            let (name, id) = if spec.silent {
                (String::from("(no device)"), None)
            } else {
                resolve(if spec.loopback { Flow::Render } else { Flow::Capture }, &spec.device)
            };
            strips.push(NodeInfo {
                kind: "input",
                name: spec.name.clone().unwrap_or(name),
                device_id: id,
                format: in_formats[i],
                channel: spec.channel,
                error: in_errors[i].clone(),
            });
        }
        strips.push(NodeInfo { kind: "tone", name: "Tone generator".into(), device_id: None, format: None, channel: None, error: None });
        let buses: Vec<NodeInfo> = cfg
            .outputs
            .iter()
            .enumerate()
            .map(|(b, dev)| {
                let (name, id) = resolve(Flow::Render, dev);
                NodeInfo { kind: "bus", name, device_id: id, format: out_formats[b], channel: None, error: None }
            })
            .collect();

        let rec_dir = cfg.rec_dir.clone().unwrap_or_else(rec::default_dir);
        let layout = rec::Layout::new(strips.len(), buses.len(), n_in);
        let rec = Arc::new(RecController::new(slot, master_fmt.sample_rate, rec_dir, layout));

        let tapper = AppTapper { tx: tap_tx, master: master_fmt, stats: stats.clone(), open: Arc::default() };
        playback.sample_rate.store(master_fmt.sample_rate as u64, Ordering::Relaxed);
        Ok(Engine { params, stats, info: EngineInfo { strips, buses }, rec, analysis, tapper, mic_test, playback, stop, threads })
    }

    pub fn stop_flag(&self) -> Arc<AtomicBool> {
        self.stop.clone()
    }

    pub fn stop(self) {
        // Finalize an in-progress take while the audio thread can still detach.
        if let Ok(s) = self.rec.stop() {
            println!("recording saved: {} ({:.1} s, {} dropped frames)", s.path.display(), s.seconds, s.dropped_frames);
        }
        self.tapper.stop_all();
        self.stop.store(true, Ordering::Relaxed);
        for t in self.threads {
            let _ = t.join();
        }
    }
}

/// Sleeps up to `ms` in short slices; returns true if the engine was asked to stop meanwhile.
fn stop_requested_within(stop: &AtomicBool, ms: u64) -> bool {
    let end = std::time::Instant::now() + std::time::Duration::from_millis(ms);
    while std::time::Instant::now() < end {
        if stop.load(Ordering::Relaxed) {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    stop.load(Ordering::Relaxed)
}

/// After a device stream dies, keeps trying to reopen it every second. It only
/// counts as back when it offers the same rate and channel count the graph was
/// built for. Returns `None` once the engine is stopping.
fn reopen<T>(
    stop: &AtomicBool,
    want: StreamFormat,
    open: impl Fn() -> audiora_hal::Result<T>,
    format_of: impl Fn(&T) -> StreamFormat,
) -> Option<T> {
    loop {
        if stop_requested_within(stop, 1000) {
            return None;
        }
        if let Ok(s) = open() {
            let f = format_of(&s);
            if f.sample_rate == want.sample_rate && f.channels == want.channels {
                eprintln!("device is back");
                return Some(s);
            }
        }
    }
}

/// Frames to keep buffered between two clock domains: one producer period plus
/// one consumer period, with 50 % margin, expressed in producer-rate frames.
fn target_frames(prod: &StreamFormat, cons: &StreamFormat) -> usize {
    let cons_in_prod_rate = cons.period_frames as f64 * prod.sample_rate as f64 / cons.sample_rate as f64;
    ((prod.period_frames as f64 + cons_in_prod_rate) * 1.5) as usize
}

struct MasterArgs {
    device: Option<String>,
    fallback: Option<MasterFallback>,
    mode: Mode,
    tone_hz: Option<f32>,
    in_cons: Vec<Option<ring::Consumer>>,
    in_formats: Vec<Option<StreamFormat>>,
    /// Loopback inputs deliver nothing while their device is idle: running dry is silence, not a glitch.
    in_loopback: Vec<bool>,
    extra_prods: Vec<ring::Producer>,
    rate_txs: Vec<mpsc::Sender<StreamFormat>>,
    n_buses: usize,
    analysis: Arc<Analysis>,
    taps: mpsc::Receiver<AppTap>,
    mic_test: Arc<MicTest>,
    playback: Arc<Playback>,
    play_cons: ring::Consumer,
}

fn master_thread(
    mut a: MasterArgs,
    stop: Arc<AtomicBool>,
    stats: Arc<Stats>,
    slot: Arc<TapSlot>,
    ready: mpsc::Sender<Result<(StreamFormat, Arc<MixerParams>), String>>,
) {
    let stream = match RenderStream::open(a.device.as_deref(), a.mode) {
        Ok(s) => s,
        Err(e) => {
            let _ = ready.send(Err(e.to_string()));
            return;
        }
    };
    let fmt = stream.format();
    let n_in = a.in_cons.len();
    let n_strips = n_in + 1;
    let tone_idx = n_in;
    let (mut mixer, params) = Mixer::new(fmt.sample_rate as f32, n_strips, a.n_buses);
    params.strips[tone_idx].gain_db.set(-30.0);
    // Capture devices start unrouted: a microphone sent straight to speakers or
    // headphones would feed back or echo the user's own voice. They are routed
    // explicitly (routing matrix / strip buttons) or by a restored session.
    for strip in &params.strips[..n_in] {
        strip.bus_mask.store(0, Ordering::Relaxed);
    }
    params.buses[0].limiter.enabled.store(true, Ordering::Relaxed);
    params.buses[0].limiter.ceiling_db.set(-1.0);
    for tx in &a.rate_txs {
        let _ = tx.send(fmt);
    }
    let _ = ready.send(Ok((fmt, params)));

    let cap = (fmt.buffer_frames as usize).max(MAX_BLOCK_FRAMES) * 2;
    let mut resamplers: Vec<Option<DriftResampler>> = a
        .in_formats
        .iter()
        .map(|f| f.map(|f| DriftResampler::new(f.sample_rate, fmt.sample_rate, target_frames(&f, &fmt))))
        .collect();
    let mut in_bufs: Vec<Vec<f32>> = (0..n_strips).map(|_| vec![0.0; cap]).collect();
    let mut bus_bufs: Vec<Vec<f32>> = (0..a.n_buses).map(|_| vec![0.0; cap]).collect();
    let ch = fmt.channels as usize;
    let tone_step = a.tone_hz.map(|hz| TAU * hz / fmt.sample_rate as f32);
    let mut phase = 0f32;
    let mut tap = AudioTap::new(slot, cap / 2, 2 * n_strips + a.n_buses);
    let (mut analysis_tap, analysis_thread) =
        analysis::spawn(a.analysis.clone(), fmt.sample_rate as f32, cap / 2, stop.clone());
    let mut taps: Vec<AppTap> = Vec::with_capacity(MAX_APP_TAPS);
    let mt = a.mic_test.clone();
    mt.rate.store(fmt.sample_rate as usize, Ordering::Relaxed);
    let mut mic_buf = vec![0f32; MIC_TEST_MAX_SECS * fmt.sample_rate as usize * 2];
    let silence = vec![0f32; cap];
    let mut play_buf = vec![0f32; cap];
    let pb = a.playback.clone();
    let mut tap_buf = vec![0f32; cap];
    // finished taps are freed on this helper thread, never on the audio thread
    let (reaper_tx, reaper_rx) = mpsc::sync_channel::<AppTap>(MAX_APP_TAPS);
    std::thread::spawn(move || for dead in reaper_rx { drop(dead) });

    let mut render = |out: &mut [f32], frames: usize| {
        let n = frames * 2;
        stats.callbacks.fetch_add(1, Ordering::Relaxed);
        tap.poll();

        for i in 0..n_in {
            let (Some(rs), Some(cons)) = (resamplers[i].as_mut(), a.in_cons[i].as_mut()) else {
                in_bufs[i][..n].fill(0.0);
                continue;
            };
            let r = rs.fill(cons, &mut in_bufs[i][..n]);
            if r.underrun && !a.in_loopback[i] {
                stats.underruns.fetch_add(1, Ordering::Relaxed);
            }
            if r.dropped {
                stats.drift_drops.fetch_add(1, Ordering::Relaxed);
            }
        }
        while taps.len() < MAX_APP_TAPS {
            match a.taps.try_recv() {
                Ok(t) => taps.push(t),
                Err(_) => break,
            }
        }
        let mut k = 0;
        while k < taps.len() {
            if taps[k].stop.load(Ordering::Relaxed) {
                let _ = reaper_tx.try_send(taps.swap_remove(k));
                continue;
            }
            let t = &mut taps[k];
            if t.strip < n_in {
                // running dry is the application being quiet, not a glitch
                let r = t.rs.fill(&mut t.cons, &mut tap_buf[..n]);
                if r.dropped {
                    stats.drift_drops.fetch_add(1, Ordering::Relaxed);
                }
                for (o, v) in in_bufs[t.strip][..n].iter_mut().zip(&tap_buf[..n]) {
                    *o += *v;
                }
            }
            k += 1;
        }
        match tone_step {
            Some(step) => {
                for f in in_bufs[tone_idx][..n].chunks_exact_mut(2) {
                    let s = phase.sin();
                    f[0] = s;
                    f[1] = s;
                    phase = (phase + step) % TAU;
                }
            }
            None => in_bufs[tone_idx][..n].fill(0.0),
        }

        // microphone check: commands first, so a new recording includes this block
        match mt.cmd.swap(0, Ordering::Acquire) {
            1 => {
                let strip = mt.strip.load(Ordering::Relaxed);
                if strip < n_strips {
                    mt.pos.store(0, Ordering::Relaxed);
                    mt.state.store(1, Ordering::Relaxed);
                }
            }
            2 if mt.len.load(Ordering::Relaxed) > 0 => {
                mt.pos.store(0, Ordering::Relaxed);
                mt.state.store(2, Ordering::Relaxed);
            }
            3 => {
                if mt.state.load(Ordering::Relaxed) == 1 {
                    mt.len.store(mt.pos.load(Ordering::Relaxed), Ordering::Relaxed);
                }
                mt.state.store(0, Ordering::Relaxed);
            }
            _ => {}
        }

        {
            let inputs: ArrayVec<&[f32], MAX_STRIPS> = in_bufs.iter().map(|b| &b[..n]).collect();
            let mut outputs: ArrayVec<&mut [f32], MAX_BUSES> =
                bus_bufs.iter_mut().map(|b| &mut b[..n]).collect();
            mixer.process(&inputs, &mut outputs);
        }

        match mt.state.load(Ordering::Relaxed) {
            1 => {
                let tap = mixer.post(mt.strip.load(Ordering::Relaxed));
                let pos = mt.pos.load(Ordering::Relaxed);
                let limit = mt.limit.load(Ordering::Relaxed).min(mic_buf.len() / 2);
                let take = (tap.len() / 2).min(limit.saturating_sub(pos));
                mic_buf[pos * 2..(pos + take) * 2].copy_from_slice(&tap[..take * 2]);
                mt.pos.store(pos + take, Ordering::Relaxed);
                if pos + take >= limit {
                    mt.len.store(pos + take, Ordering::Relaxed);
                    mt.state.store(0, Ordering::Relaxed);
                }
            }
            2 => {
                let pos = mt.pos.load(Ordering::Relaxed);
                let len = mt.len.load(Ordering::Relaxed);
                let take = frames.min(len.saturating_sub(pos));
                for (o, v) in bus_bufs[0][..take * 2].iter_mut().zip(&mic_buf[pos * 2..(pos + take) * 2]) {
                    *o += *v;
                }
                mt.pos.store(pos + take, Ordering::Relaxed);
                if pos + take >= len {
                    mt.state.store(0, Ordering::Relaxed);
                }
            }
            _ => {}
        }

        // Studio playback joins the master after the mixer
        if pb.flush.swap(false, Ordering::Acquire) {
            while a.play_cons.pop_slice(&mut play_buf[..]) > 0 {}
        }
        let got = a.play_cons.pop_slice(&mut play_buf[..n]);
        if got > 0 {
            for (o, v) in bus_bufs[0][..got].iter_mut().zip(&play_buf[..got]) {
                *o += *v;
            }
            pb.frames.fetch_add((got / 2) as u64, Ordering::Relaxed);
        }

        for (prod, bus) in a.extra_prods.iter_mut().zip(&bus_bufs[1..]) {
            if prod.push_slice(&bus[..n]) < n {
                stats.capture_drops.fetch_add(1, Ordering::Relaxed);
            }
        }

        // recordable sources: raw strips, buses, then every strip's finished signal (Layout order)
        let mut sources: ArrayVec<&[f32], { 2 * MAX_STRIPS + MAX_BUSES }> =
            in_bufs.iter().chain(bus_bufs.iter()).map(|b| &b[..n]).collect();
        for i in 0..n_strips {
            let p = mixer.post(i);
            sources.push(if p.len() >= n { &p[..n] } else { &silence[..n] });
        }
        tap.push(frames, &sources);
        analysis_tap.push(&sources, &bus_bufs[0][..n]);

        for (i, frame) in out.chunks_exact_mut(ch).enumerate() {
            frame[0] = bus_bufs[0][i * 2];
            if ch > 1 {
                frame[1] = bus_bufs[0][i * 2 + 1];
            }
        }
    };

    let mut stream = stream;
    loop {
        let result = stream.run_until(&stop, None, &mut render);
        if stop.load(Ordering::Relaxed) {
            break;
        }
        stats.device_errors.fetch_add(1, Ordering::Relaxed);
        eprintln!("master output lost ({}); retrying", result.err().map_or_else(|| "stream ended".into(), |e| e.to_string()));
        // Give the device three tries to come back (a Bluetooth hiccup), then follow the fallback.
        let attempts = std::cell::Cell::new(0u32);
        let open = || {
            attempts.set(attempts.get() + 1);
            match (RenderStream::open(a.device.as_deref(), a.mode), &a.fallback) {
                (Ok(s), _) => Ok(s),
                (Err(e), Some(pick)) if attempts.get() >= 3 => match pick().filter(|id| Some(id) != a.device.as_ref()) {
                    Some(id) => {
                        let s = RenderStream::open(Some(&id), a.mode)?;
                        eprintln!("master moved to {id}");
                        Ok(s)
                    }
                    None => Err(e),
                },
                (Err(e), _) => Err(e),
            }
        };
        match reopen(&stop, fmt, open, |s| s.format()) {
            Some(s) => stream = s,
            None => break,
        }
    }
    stop.store(true, Ordering::Relaxed);
    let _ = analysis_thread.join();
}

fn output_thread(
    device: Option<String>,
    mode: Mode,
    mut cons: ring::Consumer,
    master_rx: mpsc::Receiver<StreamFormat>,
    stop: Arc<AtomicBool>,
    stats: Arc<Stats>,
    ready: mpsc::Sender<Result<StreamFormat, String>>,
) {
    let stream = match RenderStream::open(device.as_deref(), mode) {
        Ok(s) => s,
        Err(e) => {
            let _ = ready.send(Err(e.to_string()));
            return;
        }
    };
    let fmt = stream.format();
    let _ = ready.send(Ok(fmt));
    // The master reports its format once it is running; if it never does, give up.
    let Ok(master) = master_rx.recv() else { return };

    let mut rs = DriftResampler::new(master.sample_rate, fmt.sample_rate, target_frames(&master, &fmt));
    let mut scratch = vec![0f32; (fmt.buffer_frames as usize).max(MAX_BLOCK_FRAMES) * 2];
    let ch = fmt.channels as usize;
    let mut render = |out: &mut [f32], frames: usize| {
        let r = rs.fill(&mut cons, &mut scratch[..frames * 2]);
        if r.underrun {
            stats.underruns.fetch_add(1, Ordering::Relaxed);
        }
        if r.dropped {
            stats.drift_drops.fetch_add(1, Ordering::Relaxed);
        }
        for (i, frame) in out.chunks_exact_mut(ch).enumerate() {
            frame[0] = scratch[i * 2];
            if ch > 1 {
                frame[1] = scratch[i * 2 + 1];
            }
        }
    };

    let mut stream = stream;
    loop {
        let result = stream.run_until(&stop, None, &mut render);
        if stop.load(Ordering::Relaxed) {
            break;
        }
        stats.device_errors.fetch_add(1, Ordering::Relaxed);
        eprintln!("output lost ({}); retrying", result.err().map_or_else(|| "stream ended".into(), |e| e.to_string()));
        match reopen(&stop, fmt, || RenderStream::open(device.as_deref(), mode), |s| s.format()) {
            Some(s) => stream = s,
            None => break,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn capture_thread(
    device: Option<String>,
    loopback: bool,
    raw: bool,
    mode: Mode,
    prod: ring::Producer,
    stop: Arc<AtomicBool>,
    stats: Arc<Stats>,
    ready: mpsc::Sender<Result<StreamFormat, String>>,
) {
    let open = || {
        if loopback {
            CaptureStream::open_loopback(device.as_deref())
        } else if raw {
            CaptureStream::open_raw(device.as_deref(), mode)
        } else {
            CaptureStream::open(device.as_deref(), mode)
        }
    };
    let stream = match open() {
        Ok(s) => s,
        Err(e) => {
            let _ = ready.send(Err(e.to_string()));
            return;
        }
    };
    let fmt = stream.format();
    let _ = ready.send(Ok(fmt));

    let ch = fmt.channels as usize;
    let prod = std::cell::RefCell::new(prod);
    let mut stereo = vec![0f32; fmt.buffer_frames as usize * 2 * 2];
    let zeros = vec![0f32; 4800 * 2];
    let push = |block: &[f32]| {
        let mut prod = prod.borrow_mut();
        if prod.free() < block.len() {
            stats.capture_drops.fetch_add(1, Ordering::Relaxed);
        } else {
            prod.push_slice(block);
        }
    };
    let mut on_audio = |data: &[f32], frames: usize| {
        if frames * 2 > stereo.len() {
            return;
        }
        for (i, f) in data.chunks_exact(ch).take(frames).enumerate() {
            stereo[i * 2] = f[0];
            stereo[i * 2 + 1] = if ch > 1 { f[1] } else { f[0] };
        }
        push(&stereo[..frames * 2]);
    };
    // A loopback tap is silent (no packets at all) while nothing plays on its device; the graph
    // still needs its samples, so wall-clock time is turned into zeros.
    let on_idle = |frames: usize| push(&zeros[..frames.min(4800) * 2]);

    let mut stream = stream;
    loop {
        let result = if loopback {
            stream.run_until_idle(&stop, &mut on_audio, Some(&on_idle))
        } else {
            stream.run_until(&stop, &mut on_audio)
        };
        if stop.load(Ordering::Relaxed) {
            break;
        }
        stats.device_errors.fetch_add(1, Ordering::Relaxed);
        eprintln!("capture lost ({}); retrying", result.err().map_or_else(|| "stream ended".into(), |e| e.to_string()));
        match reopen(&stop, fmt, open, |s| s.format()) {
            Some(s) => stream = s,
            None => break,
        }
    }
}
