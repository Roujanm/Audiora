//! Background disk writer. The audio thread pushes whole blocks into a
//! wait-free ring through [`RecProducer`]; a dedicated thread drains it into a
//! [`WavWriter`] and re-patches the header about once per second of audio.

use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use audiora_core::ring::{self, Consumer, Producer};

use crate::wav::WavWriter;

#[derive(Default)]
pub struct RecStatus {
    /// Frames durably handed to the file writer.
    pub frames_written: AtomicU64,
    /// Frames discarded because the ring was full or the writer had failed.
    pub dropped_frames: AtomicU64,
    pub failed: AtomicBool,
    pub error: Mutex<Option<String>>,
}

/// Audio-thread end. `push_block` never allocates, locks or blocks.
pub struct RecProducer {
    prod: Producer,
    channels: usize,
    status: Arc<RecStatus>,
}

impl RecProducer {
    /// Queues one block of interleaved samples. All-or-nothing: a block that
    /// does not fit is dropped whole (and counted) so channels never shift.
    pub fn push_block(&mut self, data: &[f32]) -> bool {
        if self.status.failed.load(Ordering::Relaxed) || self.prod.free() < data.len() {
            self.status
                .dropped_frames
                .fetch_add((data.len() / self.channels) as u64, Ordering::Relaxed);
            return false;
        }
        self.prod.push_slice(data);
        true
    }
}

#[derive(Debug, Clone)]
pub struct Summary {
    pub path: PathBuf,
    pub frames: u64,
    pub dropped_frames: u64,
    pub seconds: f64,
}

pub struct RecordSession {
    status: Arc<RecStatus>,
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<io::Result<u64>>>,
    path: PathBuf,
    sample_rate: u32,
}

impl RecordSession {
    /// Creates `path` (it must not exist) and starts the writer thread.
    pub fn start(
        path: &Path,
        sample_rate: u32,
        channels: u16,
        ring_seconds: f32,
    ) -> io::Result<(Self, RecProducer)> {
        let file = File::create_new(path)?;
        let wav = WavWriter::new(file, sample_rate, channels)?;
        let ring_samples = (sample_rate as f32 * ring_seconds) as usize * channels as usize;
        let (prod, cons) = ring::ring(ring_samples);
        let status = Arc::new(RecStatus::default());
        let stop = Arc::new(AtomicBool::new(false));

        let join = {
            let (status, stop) = (status.clone(), stop.clone());
            std::thread::Builder::new()
                .name("audiora-rec-writer".into())
                .spawn(move || writer_loop(wav, cons, channels as usize, sample_rate, &stop, &status))?
        };
        let session = Self { status: status.clone(), stop, join: Some(join), path: path.to_owned(), sample_rate };
        Ok((session, RecProducer { prod, channels: channels as usize, status }))
    }

    pub fn status(&self) -> &Arc<RecStatus> {
        &self.status
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Stops the writer after draining the ring, finalizes the file and
    /// reports the totals. The producer side must already be detached.
    pub fn finish(mut self) -> io::Result<Summary> {
        self.stop.store(true, Ordering::Release);
        let join = self.join.take().expect("finish called once");
        let frames = join.join().map_err(|_| io::Error::other("writer thread panicked"))??;
        Ok(Summary {
            path: self.path.clone(),
            frames,
            dropped_frames: self.status.dropped_frames.load(Ordering::Relaxed),
            seconds: frames as f64 / self.sample_rate as f64,
        })
    }
}

impl Drop for RecordSession {
    fn drop(&mut self) {
        // Dropped without finish(): still stop the thread so the file is closed.
        self.stop.store(true, Ordering::Release);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

fn writer_loop(
    mut wav: WavWriter<File>,
    mut cons: Consumer,
    channels: usize,
    sample_rate: u32,
    stop: &AtomicBool,
    status: &RecStatus,
) -> io::Result<u64> {
    let mut buf = vec![0f32; (16_384 / channels).max(1) * channels];
    let mut since_flush = 0u64;
    let fail = |e: io::Error| {
        status.failed.store(true, Ordering::Relaxed);
        *status.error.lock().unwrap() = Some(e.to_string());
        e
    };
    loop {
        // Read the flag *before* draining: once stop is set the producer is
        // gone, so one more full drain is guaranteed to see every sample.
        let stopping = stop.load(Ordering::Acquire);
        let mut got_any = false;
        loop {
            let avail = cons.available();
            if avail < channels {
                break;
            }
            let take = (avail.min(buf.len()) / channels) * channels;
            let n = cons.pop_slice(&mut buf[..take]);
            wav.write_samples(&buf[..n]).map_err(fail)?;
            let frames = (n / channels) as u64;
            status.frames_written.fetch_add(frames, Ordering::Relaxed);
            since_flush += frames;
            got_any = true;
        }
        if since_flush >= sample_rate as u64 {
            wav.flush_header().map_err(fail)?;
            since_flush = 0;
        }
        if stopping {
            break;
        }
        if !got_any {
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    let frames = wav.frames_written();
    wav.finish().map_err(fail)?;
    Ok(frames)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        std::env::temp_dir().join(format!("audiora-test-{}-{n}-{name}.wav", std::process::id()))
    }

    #[test]
    fn records_all_samples_in_order() {
        let path = tmp("order");
        let (session, mut prod) = RecordSession::start(&path, 48_000, 4, 1.0).unwrap();
        let mut expect = Vec::new();
        for block in 0..200 {
            let data: Vec<f32> = (0..64 * 4).map(|i| (block * 1000 + i) as f32 * 1e-4).collect();
            // the ring is small relative to the burst; retry like a real
            // producer would not, so just wait for room in this test
            while !prod.push_block(&data) {
                std::thread::sleep(Duration::from_millis(1));
            }
            expect.extend_from_slice(&data);
        }
        let summary = session.finish().unwrap();
        assert_eq!(summary.frames, 200 * 64);

        let mut r = hound::WavReader::open(&path).unwrap();
        assert_eq!(r.spec().channels, 4);
        let got: Vec<f32> = r.samples::<f32>().map(|s| s.unwrap()).collect();
        // dropped blocks were retried, but the counter still saw the misses
        assert_eq!(got, expect);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn oversized_blocks_are_dropped_whole_and_counted() {
        let path = tmp("drop");
        let (session, mut prod) = RecordSession::start(&path, 100, 2, 0.04).unwrap(); // ring: 8 samples
        assert!(!prod.push_block(&[0.5; 64]));
        assert!(!prod.push_block(&[0.5; 64]));
        let summary = session.finish().unwrap();
        assert_eq!(summary.frames, 0);
        assert_eq!(summary.dropped_frames, 64);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn refuses_to_overwrite_existing_file() {
        let path = tmp("exists");
        std::fs::write(&path, b"keep me").unwrap();
        assert!(RecordSession::start(&path, 48_000, 2, 1.0).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"keep me");
        std::fs::remove_file(path).ok();
    }
}
