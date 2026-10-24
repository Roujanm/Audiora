//! `audiora-svc` — Audiora engine. Runs the live graph and the control API.
//!
//! ```text
//! audiora-svc [--in ID]... [--no-input] [--out ID]... [--tone HZ] [--low-latency]
//!             [--port N] [--ws-port N] [--allow-origin URL]... [--rec-dir DIR]
//!             [--data-dir DIR] [--fresh] [--sim-r] [--secs S]
//! ```
//!
//! `--in` / `--out` can be repeated: every input becomes a mixer strip, every
//! output a bus; the first output is the master device that clocks the mixer.

mod api;
mod engine;
mod presets;
mod rec;
mod server;

use std::process::ExitCode;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use api::Api;
use engine::{Engine, EngineConfig, NodeInfo};
use presets::PresetStore;
use serde_json::{json, Value};

fn arg<T: std::str::FromStr>(args: &[String], name: &str) -> Option<T> {
    args.windows(2).find(|w| w[0] == name).and_then(|w| w[1].parse().ok())
}

fn args_all(args: &[String], name: &str) -> Vec<String> {
    args.windows(2).filter(|w| w[0] == name).map(|w| w[1].clone()).collect()
}

fn config_from_args(args: &[String]) -> EngineConfig {
    let ins = args_all(args, "--in");
    let inputs = if args.iter().any(|a| a == "--no-input") {
        vec![]
    } else if ins.is_empty() {
        vec![None]
    } else {
        ins.into_iter().map(Some).collect()
    };
    let outs = args_all(args, "--out");
    EngineConfig {
        inputs,
        outputs: if outs.is_empty() { vec![None] } else { outs.into_iter().map(Some).collect() },
        tone_hz: arg(args, "--tone"),
        rec_dir: arg::<std::path::PathBuf>(args, "--rec-dir"),
        low_latency: args.iter().any(|a| a == "--low-latency"),
    }
}

fn node_json(index: usize, n: &NodeInfo) -> Value {
    json!({
        "index": index,
        "kind": n.kind,
        "name": n.name,
        "device_id": n.device_id,
        "sample_rate": n.format.map(|f| f.sample_rate),
        "channels": n.format.map(|f| f.channels),
        "period_frames": n.format.map(|f| f.period_frames),
        "low_latency": n.format.map(|f| f.low_latency),
    })
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let port: u16 = arg(&args, "--port").unwrap_or(7878);
    let run_for: Option<f32> = arg(&args, "--secs");

    let engine = match Engine::start(config_from_args(&args)) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };
    for (i, b) in engine.info.buses.iter().enumerate() {
        if let Some(f) = b.format {
            println!("bus {i} -> {} ({} Hz, {} ch, period {} frames{})", b.name, f.sample_rate, f.channels, f.period_frames, if f.low_latency { ", low latency" } else { "" });
        }
    }
    for (i, s) in engine.info.strips.iter().enumerate() {
        if let Some(f) = s.format {
            println!("strip {i} <- {} ({} Hz, {} ch)", s.name, f.sample_rate, f.channels);
        } else {
            println!("strip {i} <- {}", s.name);
        }
    }

    let stop = engine.stop_flag();
    {
        let stop = stop.clone();
        let _ = ctrlc::set_handler(move || stop.store(true, Ordering::Relaxed));
    }

    let strips: Vec<Value> = engine.info.strips.iter().enumerate().map(|(i, n)| node_json(i, n)).collect();
    let buses: Vec<Value> = engine.info.buses.iter().enumerate().map(|(i, n)| node_json(i, n)).collect();
    let data_dir = arg::<std::path::PathBuf>(&args, "--data-dir").unwrap_or_else(presets::default_data_dir);
    let presets = Arc::new(PresetStore::new(&data_dir));
    if !args.iter().any(|a| a == "--fresh") && presets.exists(presets::LAST) {
        match presets.load(presets::LAST, &engine.params) {
            Ok((applied, skipped)) => println!("restored last session ({applied} settings, {skipped} skipped)"),
            Err(e) => eprintln!("could not restore last session: {e}"),
        }
    }

    let api = Arc::new(Api {
        params: engine.params.clone(),
        stats: engine.stats.clone(),
        rec: engine.rec.clone(),
        presets: presets.clone(),
        data_dir: data_dir.clone(),
        hw: args.iter().any(|a| a == "--sim-r").then(|| {
            println!("R Interface SIMULATOR enabled (no real hardware is being controlled)");
            std::sync::Mutex::new(Box::new(audiora_device::SimulatedR::new()) as Box<dyn audiora_device::HardwareInterface>)
        }),
        info: json!({
            "api": api::API_VERSION,
            "recordings_dir": engine.rec.dir().display().to_string(),
            "rec_sources": engine.rec.layout().list(),
            "strips": strips,
            "buses": buses,
            "hardware": args.iter().any(|a| a == "--sim-r"),
        }),
    });

    if let Some(secs) = run_for {
        let stop = stop.clone();
        let deadline = Instant::now() + Duration::from_secs_f32(secs);
        std::thread::spawn(move || {
            while Instant::now() < deadline && !stop.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(50));
            }
            stop.store(true, Ordering::Relaxed);
        });
    }

    let ws_port: u16 = arg(&args, "--ws-port").unwrap_or(7879);
    let mut origins: Vec<String> = server::DEFAULT_ORIGINS.iter().map(|s| s.to_string()).collect();
    origins.extend(args_all(&args, "--allow-origin"));
    {
        let (api, stop) = (api.clone(), stop.clone());
        std::thread::spawn(move || {
            if let Err(e) = server::serve_ws(api, ws_port, origins, stop) {
                eprintln!("websocket server error: {e}");
            }
        });
    }
    println!("control API: tcp 127.0.0.1:{port} (JSON lines), websocket 127.0.0.1:{ws_port}");
    let result = server::serve(api.clone(), port, stop);
    match presets.save(presets::LAST, &engine.params) {
        Ok(n) => println!("session saved ({n} settings)"),
        Err(e) => eprintln!("could not save session: {e}"),
    }

    let s = &engine.stats;
    println!(
        "callbacks {} | underruns {} | drift drops {} | capture drops {}",
        s.callbacks.load(Ordering::Relaxed),
        s.underruns.load(Ordering::Relaxed),
        s.drift_drops.load(Ordering::Relaxed),
        s.capture_drops.load(Ordering::Relaxed)
    );
    engine.stop();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("server error: {e}");
            ExitCode::FAILURE
        }
    }
}
