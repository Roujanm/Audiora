//! `audiora-svc` — Audiora engine. Runs the live graph and the control API.
//!
//! ```text
//! audiora-svc [--in ID]... [--no-input] [--out ID]... [--tone HZ] [--low-latency | --exclusive]
//!             [--port N] [--ws-port N] [--allow-origin URL]... [--rec-dir DIR]
//!             [--data-dir DIR] [--fresh] [--sim-r] [--secs S]
//! ```
//!
//! `--in` / `--out` can be repeated: every input becomes a mixer strip, every
//! output a bus; the first output is the master device that clocks the mixer.

mod analysis;
mod api;
mod channels;
mod cpresets;
mod engine;
mod presets;
mod rec;
mod server;
mod studio;

use std::process::ExitCode;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use api::Api;
use engine::{Engine, EngineConfig, InputSpec, NodeInfo};
use channels::{Channels, APP_CHANNELS, CHANNELS};
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
        vec![InputSpec::capture(None)]
    } else {
        ins.into_iter().map(|d| InputSpec::capture(Some(d))).collect()
    };
    let outs = args_all(args, "--out");
    EngineConfig {
        inputs,
        outputs: if outs.is_empty() { vec![None] } else { outs.into_iter().map(Some).collect() },
        tone_hz: arg(args, "--tone"),
        rec_dir: arg::<std::path::PathBuf>(args, "--rec-dir"),
        tolerant_inputs: false,
        mic_strip: None,
        master_fallback: None,
        mode: if args.iter().any(|a| a == "--exclusive") {
            audiora_hal::Mode::Exclusive
        } else if args.iter().any(|a| a == "--low-latency") {
            audiora_hal::Mode::LowLatency
        } else {
            audiora_hal::Mode::Shared
        },
    }
}

/// Channel layout: four application mixes tapped from virtual outputs, then the microphone.
fn channel_config(args: &[String], data_dir: &std::path::Path) -> (EngineConfig, channels::Resolved) {
    let devs = audiora_hal::list_devices().unwrap_or_default();
    let running = audiora_hal::running_process_names();
    let saved = channels::Config::load(data_dir);
    let mut resolved = channels::resolve(&saved, &devs, &running);
    let forced_out = args_all(args, "--out").into_iter().next();
    if let Some(out) = &forced_out {
        resolved.master_out = Some(out.clone());
        resolved.enforce_master();
    }
    let mut inputs: Vec<InputSpec> = (0..APP_CHANNELS)
        .map(|i| InputSpec {
            device: resolved.devices[i].clone(),
            loopback: true,
            silent: resolved.devices[i].is_none(),
            name: Some(CHANNELS[i].1.to_string()),
            channel: Some(CHANNELS[i].0),
            note: resolved.blocked[i].clone(),
            raw: false,
        })
        .collect();
    inputs.push(InputSpec {
        device: resolved.mic_in.clone(),
        loopback: false,
        silent: resolved.mic_in.is_none() || args.iter().any(|a| a == "--no-mic"),
        name: Some("Mic".into()),
        channel: Some("mic"),
        note: None,
        raw: saved.mic_raw,
    });
    let mut outputs = vec![resolved.master_out.clone()];
    if let Some(m) = &resolved.mic_out {
        outputs.push(Some(m.clone()));
    }
    let cfg = EngineConfig {
        inputs,
        outputs,
        tone_hz: arg(args, "--tone"),
        rec_dir: arg::<std::path::PathBuf>(args, "--rec-dir"),
        tolerant_inputs: true,
        mic_strip: Some(channels::MIC),
        // if the speakers vanish, play on whatever the automatic choice is now (never a channel's device)
        master_fallback: forced_out.is_none().then(|| {
            let dir = data_dir.to_owned();
            std::sync::Arc::new(move || {
                let devs = audiora_hal::list_devices().unwrap_or_default();
                channels::resolve(&channels::Config::load(&dir), &devs, &audiora_hal::running_process_names()).master_out
            }) as engine::MasterFallback
        }),
        mode: if args.iter().any(|a| a == "--exclusive") {
            audiora_hal::Mode::Exclusive
        } else if args.iter().any(|a| a == "--low-latency") {
            audiora_hal::Mode::LowLatency
        } else {
            audiora_hal::Mode::Shared
        },
    };
    (cfg, resolved)
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
        "exclusive": n.format.map(|f| f.exclusive),
        "bit_depth": n.format.map(|f| f.bit_depth),
        "channel": n.channel,
        "error": n.error,
    })
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let port: u16 = arg(&args, "--port").unwrap_or(7878);
    let run_for: Option<f32> = arg(&args, "--secs");

    let data_dir = arg::<std::path::PathBuf>(&args, "--data-dir").unwrap_or_else(presets::default_data_dir);
    // Channel layout unless devices were spelled out for the flat per-device layout.
    let channel_mode = !args.iter().any(|a| matches!(a.as_str(), "--in" | "--no-input" | "--legacy"));
    let (cfg, resolved) = if channel_mode {
        let (c, r) = channel_config(&args, &data_dir);
        (c, Some(r))
    } else {
        (config_from_args(&args), None)
    };
    let engine = match Engine::start(cfg) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };
    for (i, b) in engine.info.buses.iter().enumerate() {
        if let Some(f) = b.format {
            println!("bus {i} -> {} ({} Hz, {} ch, period {} frames{})", b.name, f.sample_rate, f.channels, f.period_frames, if f.exclusive { format!(", EXCLUSIVE {}-bit", f.bit_depth) } else if f.low_latency { ", low latency".to_string() } else { String::new() });
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
    let presets = Arc::new(PresetStore::new(&data_dir));
    let last = if channel_mode { presets::LAST_CHANNELS } else { presets::LAST };
    if channel_mode {
        // Application mixes feed the speakers; the microphone goes nowhere unless an output
        // was chosen for it (a mic sent to the headphones would echo the user's own voice).
        for strip in &engine.params.strips[..APP_CHANNELS] {
            strip.bus_mask.store(1, Ordering::Relaxed);
        }
        engine.params.strips[channels::MIC].bus_mask.store(if engine.info.buses.len() > 1 { 0b10 } else { 0 }, Ordering::Relaxed);
    }
    if !args.iter().any(|a| a == "--fresh") {
        let restored = if presets.exists(last) {
            Some(presets.load(last, &engine.params))
        } else if channel_mode && presets.exists(presets::LEGACY_CHANNELS) {
            // first start after the Music channel arrived: carry the old session over
            Some(presets.load_mapped(presets::LEGACY_CHANNELS, &engine.params, presets::legacy_channel_path))
        } else {
            None
        };
        match restored {
            Some(Ok((applied, skipped))) => println!("restored last session ({applied} settings, {skipped} skipped)"),
            Some(Err(e)) => eprintln!("could not restore last session: {e}"),
            None => {}
        }
    }

    let channel_state = resolved.map(|r| {
        let online: [Option<String>; APP_CHANNELS] =
            std::array::from_fn(|i| engine.info.strips[i].format.and_then(|_| engine.info.strips[i].device_id.clone()));
        let errors = engine.info.strips[..CHANNELS.len()].iter().map(|s| s.error.clone()).collect();
        Arc::new(Channels::new(&data_dir, engine.params.clone(), r, online, errors, Some(engine.tapper.clone())))
    });
    let watcher = channel_state.as_ref().map(|c| c.watch(stop.clone()));

    // Autosave: every change is on disk within two seconds, so a crash, a forced close or an update
    // never loses settings (the clean-exit save below is just the final word).
    let autosave = {
        let (presets, params, stop) = (presets.clone(), engine.params.clone(), stop.clone());
        std::thread::spawn(move || {
            let mut saved = params.settings();
            while !stop.load(Ordering::Relaxed) {
                for _ in 0..20 {
                    if stop.load(Ordering::Relaxed) {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
                let now = params.settings();
                if now != saved {
                    match presets.save(last, &params) {
                        Ok(_) => saved = now,
                        Err(e) => eprintln!("autosave failed: {e}"),
                    }
                }
            }
        })
    };

    // The Mic channel's mute is Windows' mute of the microphone. Discord, Chrome and every other
    // program read the microphone directly, so a mute inside Audiora alone would not silence them.
    // Audiora only ever changes it when its own button changes (or starts muted): a microphone the
    // user muted in Windows stays muted.
    let mic_mute = channel_mode.then(|| engine.info.strips[channels::MIC].device_id.clone()).flatten().map(|id| {
        let (params, stop) = (engine.params.clone(), stop.clone());
        std::thread::spawn(move || {
            let strip = &params.strips[channels::MIC];
            let mut applied = strip.mute.load(Ordering::Relaxed);
            if applied {
                let _ = audiora_hal::set_endpoint_mute(&id, true);
            }
            while !stop.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(80));
                let want = strip.mute.load(Ordering::Relaxed);
                if want != applied {
                    if let Err(e) = audiora_hal::set_endpoint_mute(&id, want) {
                        eprintln!("could not {} the microphone in Windows: {e}", if want { "mute" } else { "unmute" });
                    }
                    applied = want;
                }
            }
        })
    });

    // An automatic output follows Windows: plug in headphones (Windows makes them the default
    // output) and the engine moves onto them. It restarts itself for that; the desktop app starts
    // it again within a second. Only when nothing was chosen in Setup and no --out was given.
    let follow = (channel_mode && !args.iter().any(|a| a == "--out")).then(|| {
        let (stop, dir, current) = (stop.clone(), data_dir.clone(), engine.info.buses[0].device_id.clone());
        std::thread::spawn(move || {
            let mut seen = 0;
            while !stop.load(Ordering::Relaxed) {
                for _ in 0..20 {
                    if stop.load(Ordering::Relaxed) {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
                let cfg = channels::Config::load(&dir);
                if cfg.master_out.is_some() {
                    seen = 0;
                    continue;
                }
                let devs = audiora_hal::list_devices().unwrap_or_default();
                let pick = channels::resolve(&cfg, &devs, &audiora_hal::running_process_names()).master_out;
                // two readings in a row: Windows settles the default a moment after a device arrives
                seen = if pick.is_some() && pick != current { seen + 1 } else { 0 };
                if seen >= 2 {
                    println!("output changed to {}; restarting onto it", pick.unwrap_or_default());
                    stop.store(true, Ordering::Relaxed);
                }
            }
        })
    });

    let sim_r = args.iter().any(|a| a == "--sim-r") || (channel_mode && channels::Config::load(&data_dir).sim_r);
    let studio = Arc::new(studio::Studio::new(studio::default_root(), engine.rec.clone(), engine.playback.clone()));
    let api = Arc::new(Api {
        mic_test: Some(engine.mic_test.clone()),
        studio: Some(studio.clone()),
        channels: channel_state.clone(),
        params: engine.params.clone(),
        stats: engine.stats.clone(),
        rec: engine.rec.clone(),
        analysis: engine.analysis.clone(),
        presets: presets.clone(),
        stop: stop.clone(),
        data_dir: data_dir.clone(),
        hw: sim_r.then(|| {
            println!("R Interface SIMULATOR enabled (no real hardware is being controlled)");
            std::sync::Mutex::new(Box::new(audiora_device::SimulatedR::new()) as Box<dyn audiora_device::HardwareInterface>)
        }),
        info: json!({
            "api": api::API_VERSION,
            "channel_mode": channel_mode,
            "recordings_dir": engine.rec.dir().display().to_string(),
            "rec_sources": engine.rec.layout().list(),
            "strips": strips,
            "buses": buses,
            "hardware": sim_r,
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
    if let Some(c) = &channel_state {
        c.release_all();
    }
    if let Some(w) = watcher {
        let _ = w.join();
    }
    let _ = autosave.join();
    if let Some(m) = mic_mute {
        let _ = m.join();
    }
    if let Some(f) = follow {
        let _ = f.join();
    }
    // finish a Studio take before the engine closes its recorder
    let _ = studio.stop();
    match presets.save(last, &engine.params) {
        Ok(n) => println!("session saved ({n} settings)"),
        Err(e) => eprintln!("could not save session: {e}"),
    }

    let s = &engine.stats;
    println!(
        "callbacks {} | underruns {} | drift drops {} | capture drops {} | device errors {}",
        s.callbacks.load(Ordering::Relaxed),
        s.underruns.load(Ordering::Relaxed),
        s.drift_drops.load(Ordering::Relaxed),
        s.capture_drops.load(Ordering::Relaxed),
        s.device_errors.load(Ordering::Relaxed)
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
