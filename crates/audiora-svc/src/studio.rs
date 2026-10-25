//! Studio: multitrack projects recorded from any part of the system.
//!
//! A project is a folder `Music\Audiora\Studio\<name>` holding `project.json` and a `takes`
//! folder. Each take is one multichannel WAV written by the recorder (one stereo lane per armed
//! track); tracks place *clips* — a lane of a take at a timeline position — so editing never
//! touches the recorded audio. Playback mixes the clips on a helper thread into the engine's
//! playback ring; export renders the same mix to a file.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use serde_json::{json, Value};

use crate::engine::Playback;
use crate::rec::{wav_info, RecController};

/// Frames rendered per player step.
const BLOCK: usize = 1024;
/// Waveform overview resolution: one min/max pair per this many frames.
pub const PEAK_BIN: usize = 512;

pub fn default_root() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .map(|p| PathBuf::from(p).join("Music").join("Audiora").join("Studio"))
        .unwrap_or_else(|| std::env::temp_dir().join("Audiora").join("Studio"))
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.chars().count() <= 60
        && !name.starts_with(['.', ' '])
        && name.chars().all(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_'))
}

fn valid_take(take: &str) -> bool {
    take.ends_with(".wav") && !take.contains(['/', '\\', ':']) && !take.starts_with('.')
}

fn default_project(name: &str) -> Value {
    let track = |id: &str, name: &str, source: &str, color: &str, armed: bool| {
        json!({ "id": id, "name": name, "source": source, "color": color, "gain_db": 0.0, "mute": false, "solo": false, "armed": armed, "clips": [] })
    };
    json!({
        "version": 1,
        "name": name,
        "tracks": [
            track("t1", "Voice", "post/4", "#2ed3e6", true),
            track("t2", "Game", "post/0", "#4d9bff", false),
            track("t3", "Chat", "post/1", "#35e0a1", false),
            track("t4", "Media", "post/2", "#ff7a9c", false),
        ],
    })
}

/// One clip ready to be mixed.
#[derive(Clone)]
struct Clip {
    track: String,
    path: PathBuf,
    header: u64,
    channels: u16,
    lane: u16,
    start: u64,
    offset: u64,
    length: u64,
}

#[derive(Clone, Copy)]
struct TrackMix {
    gain: f32,
    audible: bool,
}

fn mix_of(project: &Value) -> HashMap<String, TrackMix> {
    let tracks = project["tracks"].as_array().cloned().unwrap_or_default();
    let any_solo = tracks.iter().any(|t| t["solo"].as_bool() == Some(true));
    tracks
        .iter()
        .filter_map(|t| {
            let id = t["id"].as_str()?.to_owned();
            let mute = t["mute"].as_bool() == Some(true);
            let solo = t["solo"].as_bool() == Some(true);
            let gain = 10f32.powf(t["gain_db"].as_f64().unwrap_or(0.0) as f32 / 20.0);
            Some((id, TrackMix { gain, audible: !mute && (!any_solo || solo) }))
        })
        .collect()
}

/// Mixes the clips of `[pos, pos + frames)` into `out` (interleaved stereo, overwritten).
fn render(clips: &[Clip], mix: &HashMap<String, TrackMix>, files: &mut HashMap<PathBuf, File>, pos: u64, out: &mut [f32], bytes: &mut Vec<u8>) {
    out.fill(0.0);
    let frames = (out.len() / 2) as u64;
    for c in clips {
        let Some(m) = mix.get(&c.track).copied().filter(|m| m.audible && m.gain > 0.0) else { continue };
        let s = pos.max(c.start);
        let e = (pos + frames).min(c.start + c.length);
        if s >= e {
            continue;
        }
        let n = (e - s) as usize;
        let ch = c.channels as usize;
        let src = c.offset + (s - c.start);
        let Some(file) = files.get_mut(&c.path) else { continue };
        bytes.resize(n * ch * 4, 0);
        if file.seek(SeekFrom::Start(c.header + src * ch as u64 * 4)).is_err() {
            continue;
        }
        let got = read_full(file, bytes);
        let whole = got / (ch * 4);
        let base = (s - pos) as usize;
        let lane = c.lane as usize * 2;
        for f in 0..whole.min(n) {
            let at = (f * ch + lane) * 4;
            let l = f32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
            let r = f32::from_le_bytes(bytes[at + 4..at + 8].try_into().unwrap());
            out[(base + f) * 2] += l * m.gain;
            out[(base + f) * 2 + 1] += r * m.gain;
        }
    }
}

fn read_full(file: &mut File, buf: &mut [u8]) -> usize {
    let mut done = 0;
    while done < buf.len() {
        match file.read(&mut buf[done..]) {
            Ok(0) | Err(_) => break,
            Ok(n) => done += n,
        }
    }
    done
}

struct Run {
    stop: Arc<AtomicBool>,
    thread: JoinHandle<()>,
    start: u64,
    base: u64,
    rendered: Arc<AtomicU64>,
    finished: Arc<AtomicBool>,
}

struct Recording {
    project: String,
    take: String,
    lanes: Vec<String>,
    start: u64,
}

pub struct Studio {
    root: PathBuf,
    rec: Arc<RecController>,
    playback: Arc<Playback>,
    run: Mutex<Option<Run>>,
    recording: Mutex<Option<Recording>>,
    mix: Arc<Mutex<HashMap<String, TrackMix>>>,
}

impl Studio {
    pub fn new(root: PathBuf, rec: Arc<RecController>, playback: Arc<Playback>) -> Self {
        Self { root, rec, playback, run: Mutex::new(None), recording: Mutex::new(None), mix: Arc::default() }
    }

    fn dir(&self, name: &str) -> Result<PathBuf, String> {
        if !valid_name(name) {
            return Err("project names may use letters, digits, space, - and _ (max 60)".into());
        }
        Ok(self.root.join(name))
    }

    fn rate(&self) -> u64 {
        self.playback.sample_rate.load(Ordering::Relaxed).max(1)
    }

    pub fn list(&self) -> Value {
        let mut out: Vec<(u128, String)> = fs::read_dir(&self.root)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let file = e.path().join("project.json");
                let t = fs::metadata(&file).ok()?.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_millis();
                Some((t, e.file_name().to_string_lossy().into_owned()))
            })
            .collect();
        out.sort_by_key(|p| std::cmp::Reverse(p.0));
        json!({ "projects": out.into_iter().map(|(t, n)| json!({ "name": n, "modified_ms": t as u64 })).collect::<Vec<_>>() })
    }

    pub fn load(&self, name: &str) -> Result<Value, String> {
        let dir = self.dir(name)?;
        let file = dir.join("project.json");
        let project = match fs::read_to_string(&file) {
            Ok(t) => serde_json::from_str(&t).map_err(|e| format!("corrupt project: {e}"))?,
            Err(_) => {
                let p = default_project(name);
                self.write(&dir, &p)?;
                p
            }
        };
        *self.mix.lock().unwrap() = mix_of(&project);
        Ok(json!({ "project": project, "sample_rate": self.rate(), "dir": dir.display().to_string() }))
    }

    fn write(&self, dir: &Path, project: &Value) -> Result<(), String> {
        fs::create_dir_all(dir.join("takes")).map_err(|e| e.to_string())?;
        let file = dir.join("project.json");
        let tmp = file.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_vec_pretty(project).unwrap()).map_err(|e| e.to_string())?;
        fs::rename(&tmp, &file).map_err(|e| e.to_string())
    }

    /// Stores the project the app edited (tracks, clips, mix) and applies its mix live.
    pub fn save(&self, name: &str, project: &Value) -> Result<Value, String> {
        if !project["tracks"].is_array() {
            return Err("project has no tracks".into());
        }
        let dir = self.dir(name)?;
        self.write(&dir, project)?;
        *self.mix.lock().unwrap() = mix_of(project);
        Ok(json!({ "saved": true }))
    }

    fn project(&self, name: &str) -> Result<Value, String> {
        let t = fs::read_to_string(self.dir(name)?.join("project.json")).map_err(|_| format!("no project named \"{name}\""))?;
        serde_json::from_str(&t).map_err(|e| format!("corrupt project: {e}"))
    }

    fn clips(&self, name: &str, project: &Value, skip: &[String]) -> Result<(Vec<Clip>, u64), String> {
        let takes = self.dir(name)?.join("takes");
        let mut info: HashMap<String, (u16, u64)> = HashMap::new();
        let mut clips = Vec::new();
        let mut end = 0;
        for t in project["tracks"].as_array().into_iter().flatten() {
            let Some(id) = t["id"].as_str() else { continue };
            if skip.iter().any(|s| s == id) {
                continue;
            }
            for c in t["clips"].as_array().into_iter().flatten() {
                let Some(take) = c["take"].as_str().filter(|t| valid_take(t)) else { continue };
                let path = takes.join(take);
                let (channels, header) = match info.get(take) {
                    Some(v) => *v,
                    None => {
                        let Some((ch, _, header)) = wav_info(&path) else { continue };
                        info.insert(take.to_owned(), (ch, header));
                        (ch, header)
                    }
                };
                let lane = c["lane"].as_u64().unwrap_or(0) as u16;
                if (lane as usize + 1) * 2 > channels as usize {
                    continue;
                }
                let clip = Clip {
                    track: id.to_owned(),
                    path,
                    header,
                    channels,
                    lane,
                    start: c["start"].as_u64().unwrap_or(0),
                    offset: c["offset"].as_u64().unwrap_or(0),
                    length: c["length"].as_u64().unwrap_or(0),
                };
                end = end.max(clip.start + clip.length);
                clips.push(clip);
            }
        }
        Ok((clips, end))
    }

    fn stop_run(&self) {
        if let Some(run) = self.run.lock().unwrap().take() {
            run.stop.store(true, Ordering::Relaxed);
            let _ = run.thread.join();
        }
        self.flush();
    }

    /// Drops everything queued for playback and waits until the audio thread has done so.
    fn flush(&self) {
        self.playback.flush.store(true, Ordering::Release);
        for _ in 0..50 {
            if !self.playback.flush.load(Ordering::Acquire) {
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    /// Plays the project from timeline frame `start`, leaving out the tracks in `skip`.
    pub fn play(&self, name: &str, start: u64, skip: &[String]) -> Result<Value, String> {
        self.stop_run();
        let project = self.project(name)?;
        let (clips, end) = self.clips(name, &project, skip)?;
        *self.mix.lock().unwrap() = mix_of(&project);
        let mut prod = self.playback.producer.lock().unwrap().take().ok_or("playback is busy")?;
        let mut files: HashMap<PathBuf, File> = HashMap::new();
        for c in &clips {
            if !files.contains_key(&c.path) {
                if let Ok(f) = File::open(&c.path) {
                    files.insert(c.path.clone(), f);
                }
            }
        }
        let stop = Arc::new(AtomicBool::new(false));
        let rendered = Arc::new(AtomicU64::new(0));
        let finished = Arc::new(AtomicBool::new(false));
        let (stop2, rendered2, finished2, mix, playback) = (stop.clone(), rendered.clone(), finished.clone(), self.mix.clone(), self.playback.clone());
        let base = self.playback.frames.load(Ordering::Relaxed);
        let thread = std::thread::spawn(move || {
            let mut out = vec![0f32; BLOCK * 2];
            let mut bytes = Vec::new();
            let mut pos = start;
            while !stop2.load(Ordering::Relaxed) && pos < end {
                if prod.free() < out.len() {
                    std::thread::sleep(Duration::from_millis(4));
                    continue;
                }
                let n = ((end - pos) as usize).min(BLOCK);
                let m = mix.lock().map(|m| m.clone()).unwrap_or_default();
                render(&clips, &m, &mut files, pos, &mut out[..n * 2], &mut bytes);
                prod.push_slice(&out[..n * 2]);
                pos += n as u64;
                rendered2.fetch_add(n as u64, Ordering::Relaxed);
            }
            finished2.store(true, Ordering::Relaxed);
            *playback.producer.lock().unwrap() = Some(prod);
        });
        *self.run.lock().unwrap() = Some(Run { stop, thread, start, base, rendered, finished });
        Ok(json!({ "playing": true, "start": start, "end": end }))
    }

    /// Starts recording the given tracks (`[{id, source}]`) into a new take at timeline frame
    /// `start`, playing the other tracks meanwhile so new parts can be recorded against them.
    pub fn record(&self, name: &str, tracks: &[(String, String)], start: u64) -> Result<Value, String> {
        if tracks.is_empty() {
            return Err("arm at least one track".into());
        }
        if self.recording.lock().unwrap().is_some() {
            return Err("already recording".into());
        }
        let dir = self.dir(name)?;
        fs::create_dir_all(dir.join("takes")).map_err(|e| e.to_string())?;
        let stamp = chrono::Local::now().format("take-%Y%m%d-%H%M%S");
        let mut take = format!("{stamp}.wav");
        let mut n = 2;
        while dir.join("takes").join(&take).exists() {
            take = format!("{stamp}-{n}.wav");
            n += 1;
        }
        let ids: Vec<String> = tracks.iter().map(|(id, _)| id.clone()).collect();
        let sources: Vec<String> = tracks.iter().map(|(_, s)| s.clone()).collect();
        let _ = self.play(name, start, &ids);
        if let Err(e) = self.rec.start_at(&sources, Some(dir.join("takes").join(&take))) {
            self.stop_run();
            return Err(e);
        }
        *self.recording.lock().unwrap() = Some(Recording { project: name.to_owned(), take: take.clone(), lanes: ids.clone(), start });
        Ok(json!({ "take": take, "lanes": ids, "start": start }))
    }

    /// Stops playback and recording. A finished take comes back with its length so the app can
    /// place its clips; its waveform overview is computed right away.
    pub fn stop(&self) -> Result<Value, String> {
        self.stop_run();
        let Some(r) = self.recording.lock().unwrap().take() else { return Ok(json!({ "take": Value::Null })) };
        let summary = self.rec.stop()?;
        let path = self.dir(&r.project)?.join("takes").join(&r.take);
        let _ = compute_peaks(&path);
        Ok(json!({ "take": r.take, "lanes": r.lanes, "start": r.start, "frames": summary.frames, "dropped_frames": summary.dropped_frames }))
    }

    pub fn status(&self) -> Value {
        let rate = self.rate();
        let recording = self.recording.lock().unwrap();
        if let Some(r) = recording.as_ref() {
            return json!({ "state": "recording", "position": r.start + self.rec.frames(), "sample_rate": rate, "take": r.take });
        }
        drop(recording);
        let run = self.run.lock().unwrap();
        match run.as_ref() {
            Some(run) => {
                let played = self.playback.frames.load(Ordering::Relaxed).saturating_sub(run.base);
                let done = run.finished.load(Ordering::Relaxed) && played >= run.rendered.load(Ordering::Relaxed);
                json!({ "state": if done { "stopped" } else { "playing" }, "position": run.start + played, "sample_rate": rate })
            }
            None => json!({ "state": "stopped", "position": Value::Null, "sample_rate": rate }),
        }
    }

    pub fn peaks(&self, name: &str, take: &str) -> Result<Value, String> {
        if !valid_take(take) {
            return Err("invalid take name".into());
        }
        let path = self.dir(name)?.join("takes").join(take);
        let cache = path.with_extension("peaks.json");
        if let Ok(t) = fs::read_to_string(&cache) {
            if let Ok(v) = serde_json::from_str::<Value>(&t) {
                return Ok(v);
            }
        }
        compute_peaks(&path)
    }

    /// Renders the whole project to `<project>\<name> mix.wav` (32-bit float stereo).
    pub fn export(&self, name: &str) -> Result<Value, String> {
        let project = self.project(name)?;
        let (clips, end) = self.clips(name, &project, &[])?;
        if end == 0 {
            return Err("nothing to export yet".into());
        }
        let mix = mix_of(&project);
        let mut files: HashMap<PathBuf, File> = HashMap::new();
        for c in &clips {
            if let Ok(f) = File::open(&c.path) {
                files.entry(c.path.clone()).or_insert(f);
            }
        }
        let path = self.dir(name)?.join(format!("{name} mix.wav"));
        let rate = self.rate() as u32;
        let mut w = std::io::BufWriter::new(File::create(&path).map_err(|e| e.to_string())?);
        let data_bytes = end * 2 * 4;
        if data_bytes > u32::MAX as u64 - 64 {
            return Err("the mix is too long for a WAV file".into());
        }
        write_header(&mut w, rate, data_bytes as u32).map_err(|e| e.to_string())?;
        let mut out = vec![0f32; BLOCK * 2];
        let mut bytes = Vec::new();
        let mut pos = 0;
        let mut peak = 0f32;
        while pos < end {
            let n = ((end - pos) as usize).min(BLOCK);
            render(&clips, &mix, &mut files, pos, &mut out[..n * 2], &mut bytes);
            for v in &out[..n * 2] {
                peak = peak.max(v.abs());
                w.write_all(&v.to_le_bytes()).map_err(|e| e.to_string())?;
            }
            pos += n as u64;
        }
        w.flush().map_err(|e| e.to_string())?;
        Ok(json!({ "path": path.display().to_string(), "seconds": end as f64 / rate as f64, "peak": peak }))
    }
}

fn write_header(w: &mut impl Write, rate: u32, data: u32) -> std::io::Result<()> {
    w.write_all(b"RIFF")?;
    w.write_all(&(36 + data).to_le_bytes())?;
    w.write_all(b"WAVEfmt ")?;
    w.write_all(&16u32.to_le_bytes())?;
    w.write_all(&3u16.to_le_bytes())?; // IEEE float
    w.write_all(&2u16.to_le_bytes())?;
    w.write_all(&rate.to_le_bytes())?;
    w.write_all(&(rate * 8).to_le_bytes())?;
    w.write_all(&8u16.to_le_bytes())?;
    w.write_all(&32u16.to_le_bytes())?;
    w.write_all(b"data")?;
    w.write_all(&data.to_le_bytes())
}

/// Min/max per [`PEAK_BIN`] frames for every lane of a take; cached next to it.
pub fn compute_peaks(path: &Path) -> Result<Value, String> {
    let (channels, rate, header) = wav_info(path).ok_or("not a take written by Audiora")?;
    let ch = channels as usize;
    let lanes = ch / 2;
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    file.seek(SeekFrom::Start(header)).map_err(|e| e.to_string())?;
    let mut bytes = vec![0u8; PEAK_BIN * ch * 4];
    let mut out: Vec<Vec<f32>> = vec![Vec::new(); lanes];
    let mut frames = 0u64;
    loop {
        let got = read_full(&mut file, &mut bytes) / (ch * 4);
        if got == 0 {
            break;
        }
        frames += got as u64;
        for (lane, o) in out.iter_mut().enumerate() {
            let (mut lo, mut hi) = (0f32, 0f32);
            for f in 0..got {
                for c in 0..2 {
                    let at = (f * ch + lane * 2 + c) * 4;
                    let v = f32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
                    lo = lo.min(v);
                    hi = hi.max(v);
                }
            }
            o.push((lo * 1000.0).round() / 1000.0);
            o.push((hi * 1000.0).round() / 1000.0);
        }
        if got < PEAK_BIN {
            break;
        }
    }
    let v = json!({ "bin": PEAK_BIN, "frames": frames, "sample_rate": rate, "lanes": out });
    let _ = fs::write(path.with_extension("peaks.json"), v.to_string());
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_takes_are_validated() {
        assert!(valid_name("My song 1") && !valid_name("../x") && !valid_name(""));
        assert!(valid_take("take-1.wav") && !valid_take("..\\x.wav") && !valid_take("a/b.wav") && !valid_take("x.txt"));
    }

    #[test]
    fn mix_respects_mute_solo_and_gain() {
        let p = json!({ "tracks": [
            { "id": "a", "gain_db": -6.0206, "mute": false, "solo": false },
            { "id": "b", "gain_db": 0.0, "mute": true, "solo": false },
        ]});
        let m = mix_of(&p);
        assert!((m["a"].gain - 0.5).abs() < 1e-3 && m["a"].audible && !m["b"].audible);
        let p = json!({ "tracks": [{ "id": "a", "solo": false }, { "id": "b", "solo": true }] });
        let m = mix_of(&p);
        assert!(!m["a"].audible && m["b"].audible);
    }

    #[test]
    fn render_places_clips_on_the_timeline() {
        // a fake 2-lane take: lane 0 = 0.25, lane 1 = 0.5, 100 frames
        let dir = std::env::temp_dir().join(format!("audiora-studio-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.wav");
        let mut data = Vec::new();
        for _ in 0..100 {
            for v in [0.25f32, 0.25, 0.5, 0.5] {
                data.extend_from_slice(&v.to_le_bytes());
            }
        }
        fs::write(&path, &data).unwrap();
        let clip = |lane, start| Clip { track: "a".into(), path: path.clone(), header: 0, channels: 4, lane, start, offset: 10, length: 50 };
        let clips = vec![clip(1, 20)];
        let mut mix = HashMap::new();
        mix.insert("a".to_string(), TrackMix { gain: 1.0, audible: true });
        let mut files = HashMap::new();
        files.insert(path.clone(), File::open(&path).unwrap());
        let mut out = vec![0f32; 64 * 2];
        render(&clips, &mix, &mut files, 0, &mut out, &mut Vec::new());
        assert_eq!(out[19 * 2], 0.0, "before the clip");
        assert_eq!(out[20 * 2], 0.5, "lane 1 from the clip start");
        assert_eq!(out[63 * 2 + 1], 0.5);
        fs::remove_dir_all(dir).ok();
    }
}
