//! `audiora` developer CLI.
//!
//!   audiora devices
//!   audiora tone [--device ID] [--freq HZ] [--secs S] [--gain DB] [--bass DB] [--low-latency | --exclusive]

use std::f32::consts::TAU;
use std::process::ExitCode;

use audiora_core::{Mixer, MAX_BLOCK_FRAMES};
use audiora_hal::{get_app_output, set_app_output, CaptureStream, list_devices, list_sessions, set_session, Flow, Mode, ProcessCapture, RenderStream};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("devices") => devices(),
        Some("tone") => tone(&args[1..]),
        Some("proccap") => proccap(&args[1..]),
        Some("route") => route(&args[1..]),
        Some("props") => props(&args[1..]),
        Some("loopcap") => loopcap(&args[1..]),
        _ => {
            eprintln!("usage: audiora devices | tone [--device ID] [--freq HZ] [--secs S] [--gain DB] [--bass DB]");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

type Res = Result<(), Box<dyn std::error::Error>>;

fn devices() -> Res {
    for d in list_devices()? {
        let kind = if d.flow == Flow::Render { "OUT" } else { "IN " };
        let def = if d.is_default { "*" } else { " " };
        println!("{def} {kind} {:>6} Hz {:>2} ch  {}\n         id: {}", d.sample_rate, d.channels, d.name, d.id);
    }
    Ok(())
}

fn flag<T: std::str::FromStr>(args: &[String], name: &str, default: T) -> T {
    args.windows(2)
        .find(|w| w[0] == name)
        .and_then(|w| w[1].parse().ok())
        .unwrap_or(default)
}

/// Sine -> mixer strip 0 (gain, optional bass shelf) -> bus 0 -> device.
fn tone(args: &[String]) -> Res {
    let freq: f32 = flag(args, "--freq", 440.0);
    let secs: f32 = flag(args, "--secs", 2.0);
    let gain_db: f32 = flag(args, "--gain", -20.0);
    let bass_db: f32 = flag(args, "--bass", 0.0);
    let device: String = flag(args, "--device", String::new());
    let mode = if args.iter().any(|a| a == "--exclusive") { Mode::Exclusive } else if args.iter().any(|a| a == "--low-latency") { Mode::LowLatency } else { Mode::Shared };

    let stream = if args.iter().any(|a| a == "--switchable") {
        RenderStream::open_switchable()?
    } else {
        RenderStream::open((!device.is_empty()).then_some(device.as_str()), mode)?
    };
    let fmt = stream.format();
    println!("{} Hz, {} ch, {}-bit{}, buffer {} frames (period {}); {freq} Hz at {gain_db} dB for {secs} s", fmt.sample_rate, fmt.channels, fmt.bit_depth, if fmt.exclusive { " EXCLUSIVE" } else { "" }, fmt.buffer_frames, fmt.period_frames);

    let (mut mixer, params) = Mixer::new(fmt.sample_rate as f32, 1, 1);
    params.strips[0].gain_db.set(gain_db);
    params.strips[0].eq[0].gain_db.set(bass_db);

    let cap = (fmt.buffer_frames as usize).max(MAX_BLOCK_FRAMES);
    let mut input = vec![0f32; cap * 2];
    let mut bus = vec![0f32; cap * 2];
    let ch = fmt.channels as usize;
    let step = TAU * freq / fmt.sample_rate as f32;
    let mut phase = 0f32;

    stream.run(secs, |out, frames| {
        for f in input[..frames * 2].chunks_exact_mut(2) {
            let s = phase.sin();
            f[0] = s;
            f[1] = s;
            phase = (phase + step) % TAU;
        }
        mixer.process(&[&input[..frames * 2]], &mut [&mut bus[..frames * 2]]);
        for (i, frame) in out.chunks_exact_mut(ch).enumerate() {
            frame[0] = bus[i * 2];
            if ch > 1 {
                frame[1] = bus[i * 2 + 1];
            }
        }
    })?;
    println!("peak L/R: {:.3} / {:.3}", params.buses[0].peak_l.get(), params.buses[0].peak_r.get());
    Ok(())
}

/// Captures one process tree's audio and prints its peak once a second; `--mute` also mutes the
/// process's own sessions meanwhile (restored afterwards) to prove capture ignores the mute.
fn proccap(args: &[String]) -> Res {
    let pid: u32 = args.first().and_then(|a| a.parse().ok()).ok_or("usage: proccap PID [--secs S] [--mute]")?;
    let secs: u64 = flag(args, "--secs", 5);
    let mute_vol = args.iter().any(|a| a == "--vol0");
    let mute = mute_vol || args.iter().any(|a| a == "--mute");
    let cap = ProcessCapture::open(pid)?;
    let muted: Vec<_> = if mute { list_sessions()?.into_iter().filter(|s| s.pid == pid).collect() } else { Vec::new() };
    for s in &muted {
        set_session(&s.device_id, &s.instance_id, mute_vol.then_some(0.0), (!mute_vol).then_some(true))?;
    }
    println!("capturing pid {pid}; muted {} session(s)", muted.len());
    let stop = std::sync::atomic::AtomicBool::new(false);
    let start = std::time::Instant::now();
    let (mut peak, mut frames, mut last) = (0f32, 0usize, 0u64);
    let result = cap.run_until(&stop, |buf, n| {
        frames += n;
        peak = buf.iter().fold(peak, |m, v| m.max(v.abs()));
        let t = start.elapsed().as_secs();
        if t != last {
            println!("t={t}s frames={frames} peak={peak:.4}");
            peak = 0.0;
            last = t;
            if t >= secs {
                stop.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        }
    });
    for s in &muted {
        let _ = set_session(&s.device_id, &s.instance_id, Some(s.volume), Some(false));
    }
    result?;
    Ok(())
}

fn find_render(name: &str) -> Result<String, Box<dyn std::error::Error>> {
    let n = name.to_lowercase();
    list_devices()?
        .into_iter()
        .find(|d| d.flow == Flow::Render && d.name.to_lowercase().contains(&n))
        .map(|d| d.id)
        .ok_or_else(|| format!("no output device matching '{name}'").into())
}

/// `route PID DEVICE-NAME-PART | default | get` — per-app output device.
fn route(args: &[String]) -> Res {
    let pid: u32 = args.first().and_then(|a| a.parse().ok()).ok_or("usage: route PID NAME|default|get")?;
    match args.get(1).map(String::as_str) {
        Some("get") | None => println!("pid {pid} -> {:?}", get_app_output(pid)?),
        Some("default") => {
            set_app_output(pid, None)?;
            println!("pid {pid} -> system default");
        }
        Some(name) => {
            let id = find_render(name)?;
            set_app_output(pid, Some(&id))?;
            println!("pid {pid} -> {id}; reads back {:?}", get_app_output(pid)?);
        }
    }
    Ok(())
}

/// `loopcap DEVICE-NAME-PART [--secs S]` — peak of what a render device is playing.
fn loopcap(args: &[String]) -> Res {
    use std::cell::Cell;
    let id = find_render(args.first().map(String::as_str).unwrap_or(""))?;
    let secs: u64 = flag(args, "--secs", 5);
    let cap = CaptureStream::open_loopback(Some(&id))?;
    println!("{:?}", cap.format());
    let ch = cap.format().channels as usize;
    let stop = std::sync::atomic::AtomicBool::new(false);
    let start = std::time::Instant::now();
    let (peak, audio, idle, last) = (Cell::new(0f32), Cell::new(0usize), Cell::new(0usize), Cell::new(0u64));
    let tick = || {
        let t = start.elapsed().as_secs();
        if t != last.get() {
            println!("t={t}s audio_frames={} idle_frames={} peak={:.4}", audio.get(), idle.get(), peak.get());
            peak.set(0.0);
            last.set(t);
            if t >= secs {
                stop.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        }
    };
    cap.run_until_idle(
        &stop,
        |buf, n| {
            audio.set(audio.get() + n);
            peak.set(buf.chunks_exact(ch).fold(peak.get(), |m, f| m.max(f[0].abs())));
            tick();
        },
        Some(|n: usize| {
            idle.set(idle.get() + n);
            tick();
        }),
    )?;
    Ok(())
}

fn props(args: &[String]) -> Res {
    for (k, v) in audiora_hal::dump_properties(&find_render(args.first().map(String::as_str).unwrap_or(""))?)? {
        println!("{k} = {v}");
    }
    Ok(())
}
