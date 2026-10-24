//! `audiora-svc` — Audiora engine. Runs the live graph and the control API.
//!
//!   audiora-svc [--in ID | --no-input] [--out ID] [--tone HZ] [--port N] [--secs S]

mod api;
mod engine;
mod rec;
mod server;

use std::process::ExitCode;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use api::Api;
use engine::{Engine, EngineConfig};
use serde_json::json;

fn arg<T: std::str::FromStr>(args: &[String], name: &str) -> Option<T> {
    args.windows(2).find(|w| w[0] == name).and_then(|w| w[1].parse().ok())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cfg = EngineConfig {
        input: arg(&args, "--in"),
        input_enabled: !args.iter().any(|a| a == "--no-input"),
        output: arg(&args, "--out"),
        tone_hz: arg(&args, "--tone"),
        rec_dir: arg::<std::path::PathBuf>(&args, "--rec-dir"),
    };
    let port: u16 = arg(&args, "--port").unwrap_or(7878);
    let run_for: Option<f32> = arg(&args, "--secs");

    let engine = match Engine::start(cfg) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };
    let out = engine.info.output;
    println!("output: {} Hz, {} ch, buffer {} frames", out.sample_rate, out.channels, out.buffer_frames);
    if let Some(i) = engine.info.input {
        println!("input:  {} Hz, {} ch", i.sample_rate, i.channels);
    }

    let stop = engine.stop_flag();
    {
        let stop = stop.clone();
        let _ = ctrlc::set_handler(move || stop.store(true, Ordering::Relaxed));
    }

    let api = Arc::new(Api {
        params: engine.params.clone(),
        stats: engine.stats.clone(),
        rec: engine.rec.clone(),
        info: json!({
            "recordings_dir": engine.rec.dir().display().to_string(),
            "rec_sources": rec::SOURCES,
            "api": api::API_VERSION,
            "output": { "sample_rate": out.sample_rate, "channels": out.channels, "buffer_frames": out.buffer_frames },
            "input": engine.info.input.map(|i| json!({ "sample_rate": i.sample_rate, "channels": i.channels })),
            "strips": engine.params.strips.len(),
            "buses": engine.params.buses.len(),
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

    println!("control API on 127.0.0.1:{port} (JSON lines)");
    let result = server::serve(api, port, stop);

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
