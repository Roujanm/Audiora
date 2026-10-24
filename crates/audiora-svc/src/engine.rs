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
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc};
use std::thread::JoinHandle;

use arrayvec::ArrayVec;
use audiora_core::{ring, DriftResampler, Mixer, MixerParams, MAX_BLOCK_FRAMES};
use audiora_hal::{list_devices, CaptureStream, Flow, RenderStream, StreamFormat};

use crate::rec::{self, AudioTap, RecController, TapSlot};

pub const MAX_STRIPS: usize = 16;
pub const MAX_BUSES: usize = 8;
const RING_SAMPLES: usize = 1 << 17;

#[derive(Clone, Debug, Default)]
pub struct EngineConfig {
    /// One entry per capture device (`None` = system default). Empty = no inputs.
    pub inputs: Vec<Option<String>>,
    /// One entry per output bus: the first is the master device that clocks
    /// the mixer. Must not be empty.
    pub outputs: Vec<Option<String>>,
    /// Built-in oscillator strip (always present, silent unless set).
    pub tone_hz: Option<f32>,
    /// Where takes are written; `None` = `%USERPROFILE%\Music\Audiora\Recordings`.
    pub rec_dir: Option<PathBuf>,
    /// Use the smallest shared-mode period the audio engine offers.
    pub low_latency: bool,
}

#[derive(Default)]
pub struct Stats {
    pub callbacks: AtomicU64,
    /// A device read from a ring that had run dry.
    pub underruns: AtomicU64,
    /// A ring overfilled (clock drift too large to trim) and old audio was discarded.
    pub drift_drops: AtomicU64,
    /// A producer could not enqueue (ring full).
    pub capture_drops: AtomicU64,
}

#[derive(Clone, Debug)]
pub struct NodeInfo {
    /// "input" or "tone" for strips; "bus" for buses.
    pub kind: &'static str,
    pub name: String,
    pub device_id: Option<String>,
    pub format: Option<StreamFormat>,
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
    stop: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
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
            let (dev, stop2, stats2, ll) = (cfg.outputs[b].clone(), stop.clone(), stats.clone(), cfg.low_latency);
            threads.push(std::thread::spawn(move || output_thread(dev, ll, cons, rate_rx, stop2, stats2, tx)));
            match rx.recv() {
                Ok(Ok(f)) => out_formats[b] = Some(f),
                Ok(Err(e)) => return abort(&stop, threads, format!("output {b}: {e}")),
                Err(_) => return abort(&stop, threads, format!("output {b}: thread died")),
            }
        }

        // Capture devices.
        let mut in_cons = Vec::new();
        let mut in_formats = Vec::new();
        for (i, dev) in cfg.inputs.iter().enumerate() {
            let (prod, cons) = ring::ring(RING_SAMPLES);
            let (tx, rx) = mpsc::channel::<Result<StreamFormat, String>>();
            let (dev, stop2, stats2, ll) = (dev.clone(), stop.clone(), stats.clone(), cfg.low_latency);
            threads.push(std::thread::spawn(move || capture_thread(dev, ll, prod, stop2, stats2, tx)));
            match rx.recv() {
                Ok(Ok(f)) => in_formats.push(f),
                Ok(Err(e)) => return abort(&stop, threads, format!("input {i}: {e}")),
                Err(_) => return abort(&stop, threads, format!("input {i}: thread died")),
            }
            in_cons.push(cons);
        }

        // Master render thread: opens the master device and runs the mixer.
        let (tx, rx) = mpsc::channel::<Result<(StreamFormat, Arc<MixerParams>), String>>();
        {
            let args = MasterArgs {
                device: cfg.outputs[0].clone(),
                low_latency: cfg.low_latency,
                tone_hz: cfg.tone_hz,
                in_cons,
                in_formats: in_formats.clone(),
                extra_prods,
                rate_txs,
                n_buses: n_out,
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
        for (i, dev) in cfg.inputs.iter().enumerate() {
            let (name, id) = resolve(Flow::Capture, dev);
            strips.push(NodeInfo { kind: "input", name, device_id: id, format: Some(in_formats[i]) });
        }
        strips.push(NodeInfo { kind: "tone", name: "Tone generator".into(), device_id: None, format: None });
        let buses: Vec<NodeInfo> = cfg
            .outputs
            .iter()
            .enumerate()
            .map(|(b, dev)| {
                let (name, id) = resolve(Flow::Render, dev);
                NodeInfo { kind: "bus", name, device_id: id, format: out_formats[b] }
            })
            .collect();

        let rec_dir = cfg.rec_dir.clone().unwrap_or_else(rec::default_dir);
        let layout = rec::Layout::new(strips.len(), buses.len(), n_in);
        let rec = Arc::new(RecController::new(slot, master_fmt.sample_rate, rec_dir, layout));

        Ok(Engine { params, stats, info: EngineInfo { strips, buses }, rec, stop, threads })
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

/// Frames to keep buffered between two clock domains: one producer period plus
/// one consumer period, with 50 % margin, expressed in producer-rate frames.
fn target_frames(prod: &StreamFormat, cons: &StreamFormat) -> usize {
    let cons_in_prod_rate = cons.period_frames as f64 * prod.sample_rate as f64 / cons.sample_rate as f64;
    ((prod.period_frames as f64 + cons_in_prod_rate) * 1.5) as usize
}

struct MasterArgs {
    device: Option<String>,
    low_latency: bool,
    tone_hz: Option<f32>,
    in_cons: Vec<ring::Consumer>,
    in_formats: Vec<StreamFormat>,
    extra_prods: Vec<ring::Producer>,
    rate_txs: Vec<mpsc::Sender<StreamFormat>>,
    n_buses: usize,
}

fn master_thread(
    mut a: MasterArgs,
    stop: Arc<AtomicBool>,
    stats: Arc<Stats>,
    slot: Arc<TapSlot>,
    ready: mpsc::Sender<Result<(StreamFormat, Arc<MixerParams>), String>>,
) {
    let stream = match RenderStream::open(a.device.as_deref(), a.low_latency) {
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
    let mut resamplers: Vec<DriftResampler> = a
        .in_formats
        .iter()
        .map(|f| DriftResampler::new(f.sample_rate, fmt.sample_rate, target_frames(f, &fmt)))
        .collect();
    let mut in_bufs: Vec<Vec<f32>> = (0..n_strips).map(|_| vec![0.0; cap]).collect();
    let mut bus_bufs: Vec<Vec<f32>> = (0..a.n_buses).map(|_| vec![0.0; cap]).collect();
    let ch = fmt.channels as usize;
    let tone_step = a.tone_hz.map(|hz| TAU * hz / fmt.sample_rate as f32);
    let mut phase = 0f32;
    let mut tap = AudioTap::new(slot, cap / 2, n_strips + a.n_buses);

    let result = stream.run_until(&stop, None, |out, frames| {
        let n = frames * 2;
        stats.callbacks.fetch_add(1, Ordering::Relaxed);
        tap.poll();

        for i in 0..n_in {
            let r = resamplers[i].fill(&mut a.in_cons[i], &mut in_bufs[i][..n]);
            if r.underrun {
                stats.underruns.fetch_add(1, Ordering::Relaxed);
            }
            if r.dropped {
                stats.drift_drops.fetch_add(1, Ordering::Relaxed);
            }
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

        {
            let inputs: ArrayVec<&[f32], MAX_STRIPS> = in_bufs.iter().map(|b| &b[..n]).collect();
            let mut outputs: ArrayVec<&mut [f32], MAX_BUSES> =
                bus_bufs.iter_mut().map(|b| &mut b[..n]).collect();
            mixer.process(&inputs, &mut outputs);
        }

        for (prod, bus) in a.extra_prods.iter_mut().zip(&bus_bufs[1..]) {
            if prod.push_slice(&bus[..n]) < n {
                stats.capture_drops.fetch_add(1, Ordering::Relaxed);
            }
        }

        let sources: ArrayVec<&[f32], { MAX_STRIPS + MAX_BUSES }> =
            in_bufs.iter().chain(bus_bufs.iter()).map(|b| &b[..n]).collect();
        tap.push(frames, &sources);

        for (i, frame) in out.chunks_exact_mut(ch).enumerate() {
            frame[0] = bus_bufs[0][i * 2];
            if ch > 1 {
                frame[1] = bus_bufs[0][i * 2 + 1];
            }
        }
    });
    if let Err(e) = result {
        eprintln!("master render stopped: {e}");
        stop.store(true, Ordering::Relaxed);
    }
}

fn output_thread(
    device: Option<String>,
    low_latency: bool,
    mut cons: ring::Consumer,
    master_rx: mpsc::Receiver<StreamFormat>,
    stop: Arc<AtomicBool>,
    stats: Arc<Stats>,
    ready: mpsc::Sender<Result<StreamFormat, String>>,
) {
    let stream = match RenderStream::open(device.as_deref(), low_latency) {
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
    let result = stream.run_until(&stop, None, |out, frames| {
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
    });
    if let Err(e) = result {
        eprintln!("output stopped: {e}");
        stop.store(true, Ordering::Relaxed);
    }
}

fn capture_thread(
    device: Option<String>,
    low_latency: bool,
    mut prod: ring::Producer,
    stop: Arc<AtomicBool>,
    stats: Arc<Stats>,
    ready: mpsc::Sender<Result<StreamFormat, String>>,
) {
    let stream = match CaptureStream::open(device.as_deref(), low_latency) {
        Ok(s) => s,
        Err(e) => {
            let _ = ready.send(Err(e.to_string()));
            return;
        }
    };
    let fmt = stream.format();
    let _ = ready.send(Ok(fmt));

    let ch = fmt.channels as usize;
    let mut stereo = vec![0f32; fmt.buffer_frames as usize * 2 * 2];
    let result = stream.run_until(&stop, |data, frames| {
        if frames * 2 > stereo.len() {
            return;
        }
        for (i, f) in data.chunks_exact(ch).take(frames).enumerate() {
            stereo[i * 2] = f[0];
            stereo[i * 2 + 1] = if ch > 1 { f[1] } else { f[0] };
        }
        let block = &stereo[..frames * 2];
        if prod.free() < block.len() {
            stats.capture_drops.fetch_add(1, Ordering::Relaxed);
        } else {
            prod.push_slice(block);
        }
    });
    if let Err(e) = result {
        eprintln!("capture stopped: {e}");
        stop.store(true, Ordering::Relaxed);
    }
}
