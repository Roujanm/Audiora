//! `audiora` developer CLI.
//!
//!   audiora devices
//!   audiora tone [--device ID] [--freq HZ] [--secs S] [--gain DB] [--bass DB]

use std::f32::consts::TAU;
use std::process::ExitCode;

use audiora_core::{Mixer, MAX_BLOCK_FRAMES};
use audiora_hal::{list_devices, Flow, RenderStream};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("devices") => devices(),
        Some("tone") => tone(&args[1..]),
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

    let stream = RenderStream::open((!device.is_empty()).then_some(device.as_str()))?;
    let fmt = stream.format();
    println!("{} Hz, {} ch, buffer {} frames; {freq} Hz at {gain_db} dB for {secs} s", fmt.sample_rate, fmt.channels, fmt.buffer_frames);

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
