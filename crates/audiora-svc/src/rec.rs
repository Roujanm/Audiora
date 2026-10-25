//! Recording glue between the control thread and the audio thread.
//!
//! The audio thread owns an [`AudioTap`]. To start or stop recording the
//! control thread parks a [`Tap`] in the shared slot and bumps a generation
//! counter; the audio thread picks it up with `try_lock` (never blocks). The
//! tap's heap data is only ever freed by the control side, because the ring and
//! status are still referenced by the writer/session when the audio thread
//! lets go of its handle.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use audiora_rec::{RecProducer, RecordSession, Summary};
use serde_json::{json, Value};

/// Most sources (stereo pairs) one take can contain.
pub const MAX_REC_SOURCES: usize = 8;

/// Names of everything the engine can record. Source index = position in the
/// array the audio thread passes to [`AudioTap::push`]: all strips (raw,
/// pre-fader), then all buses (post-mixer), then every strip after its chain
/// and fader (`post/N`, the processed "clean" signal).
#[derive(Clone, Debug)]
pub struct Layout {
    names: Vec<(String, usize)>,
}

impl Layout {
    pub fn new(n_strips: usize, n_buses: usize, n_inputs: usize) -> Self {
        let mut names = Vec::new();
        for i in 0..n_strips {
            names.push((format!("strip/{i}"), i));
        }
        for b in 0..n_buses {
            names.push((format!("bus/{b}"), n_strips + b));
        }
        for i in 0..n_strips {
            names.push((format!("post/{i}"), n_strips + n_buses + i));
        }
        // Friendly aliases kept from the first API version.
        if n_inputs > 0 {
            names.push(("input".into(), 0));
        }
        names.push(("tone".into(), n_inputs));
        names.push(("mix".into(), n_strips));
        Self { names }
    }

    pub fn resolve(&self, name: &str) -> Option<usize> {
        self.names.iter().find(|(n, _)| n == name).map(|(_, i)| *i)
    }

    pub fn list(&self) -> Vec<&str> {
        self.names.iter().map(|(n, _)| n.as_str()).collect()
    }
}

pub struct Tap {
    prod: RecProducer,
    sources: [usize; MAX_REC_SOURCES],
    n: usize,
}

#[derive(Default)]
pub struct TapSlot {
    generation: AtomicU64,
    applied: AtomicU64,
    /// Set when the audio thread's `AudioTap` is dropped (stream ended).
    closed: AtomicBool,
    inbox: Mutex<Option<Tap>>,
}

/// Audio-thread half.
pub struct AudioTap {
    slot: Arc<TapSlot>,
    local: Option<Tap>,
    seen: u64,
    scratch: Vec<f32>,
}

impl AudioTap {
    pub fn new(slot: Arc<TapSlot>, max_frames: usize, _n_sources: usize) -> Self {
        Self { slot, local: None, seen: 0, scratch: vec![0.0; max_frames * MAX_REC_SOURCES * 2] }
    }

    /// Cheap check once per callback; applies a pending start/stop.
    pub fn poll(&mut self) {
        let g = self.slot.generation.load(Ordering::Acquire);
        if g == self.seen {
            return;
        }
        if let Ok(mut inbox) = self.slot.inbox.try_lock() {
            self.local = inbox.take();
            self.seen = g;
            self.slot.applied.store(g, Ordering::Release);
        }
    }

    /// `inputs` are interleaved stereo blocks indexed like [`Layout`].
    pub fn push(&mut self, frames: usize, inputs: &[&[f32]]) {
        let Some(tap) = self.local.as_mut() else { return };
        let k = tap.n;
        let frames = frames.min(self.scratch.len() / (k * 2));
        for f in 0..frames {
            for (j, &src) in tap.sources[..k].iter().enumerate() {
                let o = (f * k + j) * 2;
                self.scratch[o] = inputs[src][f * 2];
                self.scratch[o + 1] = inputs[src][f * 2 + 1];
            }
        }
        tap.prod.push_block(&self.scratch[..frames * k * 2]);
    }
}

impl Drop for AudioTap {
    fn drop(&mut self) {
        self.slot.closed.store(true, Ordering::Release);
    }
}

struct Active {
    session: RecordSession,
    started: Instant,
    sources: Vec<String>,
}

/// Control-thread half.
pub struct RecController {
    slot: Arc<TapSlot>,
    sample_rate: u32,
    dir: PathBuf,
    layout: Layout,
    active: Mutex<Option<Active>>,
}

pub fn default_dir() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .map(|p| PathBuf::from(p).join("Music").join("Audiora").join("Recordings"))
        .unwrap_or_else(|| std::env::temp_dir().join("Audiora"))
}

impl RecController {
    pub fn new(slot: Arc<TapSlot>, sample_rate: u32, dir: PathBuf, layout: Layout) -> Self {
        Self { slot, sample_rate, dir, layout, active: Mutex::new(None) }
    }

    pub fn dir(&self) -> &std::path::Path {
        &self.dir
    }

    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    /// Frames written to the active take so far.
    pub fn frames(&self) -> u64 {
        self.active
            .lock()
            .ok()
            .and_then(|a| a.as_ref().map(|a| a.session.status().frames_written.load(Ordering::Relaxed)))
            .unwrap_or(0)
    }

    pub fn start(&self, requested: &[String]) -> Result<Value, String> {
        self.start_at(requested, None)
    }

    /// Like [`start`](Self::start), writing to `path` (a Studio project's take) instead of a
    /// time-stamped file in the recordings folder.
    pub fn start_at(&self, requested: &[String], path: Option<PathBuf>) -> Result<Value, String> {
        let mut active = self.active.lock().unwrap();
        if active.is_some() {
            return Err("already recording".into());
        }
        let names: Vec<String> =
            if requested.is_empty() { vec!["mix".into()] } else { requested.to_vec() };
        if names.len() > MAX_REC_SOURCES {
            return Err(format!("at most {MAX_REC_SOURCES} sources per take"));
        }
        let mut idx = [0usize; MAX_REC_SOURCES];
        for (i, name) in names.iter().enumerate() {
            let pos = self.layout.resolve(name).ok_or_else(|| {
                format!("unknown source \"{name}\" (use {:?})", self.layout.list())
            })?;
            if idx[..i].contains(&pos) {
                return Err(format!("duplicate source \"{name}\""));
            }
            idx[i] = pos;
        }

        let path = match path {
            Some(p) => p,
            None => {
                std::fs::create_dir_all(&self.dir).map_err(|e| format!("recording folder: {e}"))?;
                let stamp = chrono::Local::now().format("take-%Y%m%d-%H%M%S");
                let mut path = self.dir.join(format!("{stamp}.wav"));
                let mut n = 2;
                while path.exists() {
                    path = self.dir.join(format!("{stamp}-{n}.wav"));
                    n += 1;
                }
                path
            }
        };

        let channels = (names.len() * 2) as u16;
        let (session, prod) = RecordSession::start(&path, self.sample_rate, channels, 4.0)
            .map_err(|e| format!("cannot create {}: {e}", path.display()))?;

        *self.slot.inbox.lock().unwrap() = Some(Tap { prod, sources: idx, n: names.len() });
        self.slot.generation.fetch_add(1, Ordering::Release);

        let reply = json!({
            "path": path.display().to_string(),
            "sample_rate": self.sample_rate,
            "channels": channels,
            "sources": names,
        });
        *active = Some(Active { session, started: Instant::now(), sources: names });
        Ok(reply)
    }

    pub fn stop(&self) -> Result<Summary, String> {
        let mut active = self.active.lock().unwrap();
        let rec = active.take().ok_or("not recording")?;

        // Detach the audio thread first, then let the writer drain and close.
        *self.slot.inbox.lock().unwrap() = None;
        let g = self.slot.generation.fetch_add(1, Ordering::Release) + 1;
        let deadline = Instant::now() + Duration::from_secs(1);
        while self.slot.applied.load(Ordering::Acquire) < g
            && !self.slot.closed.load(Ordering::Acquire)
            && Instant::now() < deadline
        {
            std::thread::sleep(Duration::from_millis(2));
        }
        rec.session.finish().map_err(|e| format!("finalizing file: {e}"))
    }

    /// The newest `limit` takes in the recordings folder (this engine writes the headers, so
    /// the channel count, rate and data offset are read from fixed positions).
    pub fn list_takes(&self, limit: usize) -> Vec<Value> {
        let Ok(dir) = std::fs::read_dir(&self.dir) else { return Vec::new() };
        let mut takes: Vec<(std::time::SystemTime, Value)> = dir
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("wav")))
            .filter_map(|e| {
                let meta = e.metadata().ok()?;
                let modified = meta.modified().ok()?;
                let (channels, rate, header) = wav_info(&e.path())?;
                let bytes = meta.len();
                let seconds = bytes.saturating_sub(header) as f64 / (channels as f64 * 4.0) / rate as f64;
                let ms = modified.duration_since(std::time::UNIX_EPOCH).ok()?.as_millis() as u64;
                Some((
                    modified,
                    json!({
                        "name": e.file_name().to_string_lossy(),
                        "path": e.path().display().to_string(),
                        "bytes": bytes,
                        "modified_ms": ms,
                        "channels": channels,
                        "sample_rate": rate,
                        "seconds": seconds,
                    }),
                ))
            })
            .collect();
        takes.sort_by_key(|t| std::cmp::Reverse(t.0));
        takes.into_iter().take(limit).map(|(_, v)| v).collect()
    }

    pub fn status(&self) -> Value {
        let active = self.active.lock().unwrap();
        match active.as_ref() {
            None => json!({ "recording": false, "dir": self.dir.display().to_string() }),
            Some(a) => {
                let st = a.session.status();
                let frames = st.frames_written.load(Ordering::Relaxed);
                json!({
                    "recording": true,
                    "path": a.session.path().display().to_string(),
                    "sources": a.sources,
                    "frames": frames,
                    "seconds": frames as f64 / self.sample_rate as f64,
                    "wall_seconds": a.started.elapsed().as_secs_f64(),
                    "dropped_frames": st.dropped_frames.load(Ordering::Relaxed),
                    "error": st.error.lock().unwrap().clone(),
                })
            }
        }
    }
}

/// (channels, sample rate, header length) of a WAV written by `audiora-rec`.
pub fn wav_info(path: &std::path::Path) -> Option<(u16, u32, u64)> {
    use std::io::Read;
    let mut head = [0u8; 64];
    std::fs::File::open(path).ok()?.read_exact(&mut head).ok()?;
    if &head[8..12] != b"WAVE" || &head[48..52] != b"fmt " {
        return None;
    }
    let fmt_len = u32::from_le_bytes(head[52..56].try_into().ok()?) as u64;
    let channels = u16::from_le_bytes(head[58..60].try_into().ok()?);
    let rate = u32::from_le_bytes(head[60..64].try_into().ok()?);
    if channels == 0 || rate == 0 {
        return None;
    }
    // RIFF header (12) + JUNK chunk (36) + fmt chunk + fact chunk (12) + data chunk header (8)
    Some((channels, rate, 48 + 8 + fmt_len + 12 + 8))
}

pub fn summary_json(s: &Summary) -> Value {
    json!({
        "path": s.path.display().to_string(),
        "frames": s.frames,
        "seconds": s.seconds,
        "dropped_frames": s.dropped_frames,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_takes_with_duration_channels_and_newest_first() {
        use audiora_rec::WavWriter;
        let dir = std::env::temp_dir().join(format!("audiora-takes-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let write = |name: &str, channels: u16, frames: usize| {
            let f = std::fs::File::create_new(dir.join(name)).unwrap();
            let mut w = WavWriter::new(f, 48_000, channels).unwrap();
            w.write_samples(&vec![0.1; frames * channels as usize]).unwrap();
            w.finish().unwrap();
        };
        write("old.wav", 2, 48_000);
        std::thread::sleep(Duration::from_millis(30));
        write("new.wav", 6, 96_000);
        std::fs::write(dir.join("notes.txt"), b"ignore me").unwrap();
        std::fs::write(dir.join("broken.wav"), b"not a wav").unwrap();

        let ctl = RecController::new(Arc::new(TapSlot::default()), 48_000, dir.clone(), Layout::new(1, 1, 0));
        let takes = ctl.list_takes(10);
        assert_eq!(takes.len(), 2, "text and broken files are skipped: {takes:?}");
        assert_eq!(takes[0]["name"], "new.wav");
        assert_eq!(takes[0]["channels"], 6);
        assert!((takes[0]["seconds"].as_f64().unwrap() - 2.0).abs() < 1e-6);
        assert_eq!(takes[1]["name"], "old.wav");
        assert!((takes[1]["seconds"].as_f64().unwrap() - 1.0).abs() < 1e-6);
        assert_eq!(ctl.list_takes(1).len(), 1);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn layout_resolves_names_and_aliases() {
        let l = Layout::new(3, 2, 2); // strips 0,1 inputs, strip 2 tone; buses 3,4
        assert_eq!(l.resolve("strip/1"), Some(1));
        assert_eq!(l.resolve("bus/1"), Some(4));
        assert_eq!(l.resolve("input"), Some(0));
        assert_eq!(l.resolve("tone"), Some(2));
        assert_eq!(l.resolve("mix"), Some(3));
        assert_eq!(l.resolve("nope"), None);
        assert_eq!(Layout::new(1, 1, 0).resolve("input"), None);
    }

    #[test]
    fn handoff_records_selected_sources_into_one_multichannel_file() {
        let dir = std::env::temp_dir().join(format!("audiora-svc-test-{}", std::process::id()));
        let slot = Arc::new(TapSlot::default());
        // sources: 0 = input, 1 = tone, 2 = mix
        let ctl = RecController::new(slot.clone(), 48_000, dir.clone(), Layout::new(2, 1, 1));

        // Fake audio thread: constant per-source values.
        let run = Arc::new(AtomicBool::new(true));
        let audio = {
            let run = run.clone();
            std::thread::spawn(move || {
                let mut tap = AudioTap::new(slot, 256, 3);
                let (a, b, c) = (vec![0.25f32; 128], vec![0.5f32; 128], vec![0.75f32; 128]);
                while run.load(Ordering::Relaxed) {
                    tap.poll();
                    tap.push(64, &[&a, &b, &c]);
                    std::thread::sleep(Duration::from_millis(1));
                }
            })
        };

        assert!(ctl.start(&["bogus".into()]).is_err());
        let started = ctl.start(&["mix".into(), "input".into()]).unwrap();
        assert_eq!(started["channels"], 4);
        assert!(ctl.start(&[]).is_err(), "second start while recording");
        std::thread::sleep(Duration::from_millis(150));
        assert_eq!(ctl.status()["recording"], true);
        let summary = ctl.stop().unwrap();
        run.store(false, Ordering::Relaxed);
        audio.join().unwrap();

        assert!(summary.frames > 0 && summary.frames % 64 == 0, "whole blocks only");
        let mut r = hound::WavReader::open(&summary.path).unwrap();
        assert_eq!(r.spec().channels, 4);
        let samples: Vec<f32> = r.samples::<f32>().map(|s| s.unwrap()).collect();
        assert_eq!(samples.len() as u64, summary.frames * 4);
        for frame in samples.chunks_exact(4) {
            assert_eq!(frame, [0.75, 0.75, 0.25, 0.25], "mix first, then input");
        }
        assert!(ctl.stop().is_err());
        std::fs::remove_dir_all(dir).ok();
    }
}
