//! 32-bit IEEE float WAV writer.
//!
//! * 1-2 channels: classic `WAVE_FORMAT_IEEE_FLOAT` fmt chunk.
//! * 3+ channels: `WAVE_FORMAT_EXTENSIBLE` (unspecified speaker positions).
//! * Header sizes are re-patched on [`WavWriter::flush_header`], so a crash
//!   loses at most the audio since the last patch, never the whole file.
//! * Files that outgrow 4 GB are rewritten in place as RF64 (a reserved JUNK
//!   chunk becomes `ds64`), as defined by EBU Tech 3306.

use std::io::{self, BufWriter, Seek, SeekFrom, Write};

const JUNK_LEN: u32 = 28; // size of a ds64 chunk body without a table

/// Switch to RF64 once the RIFF payload would pass this many bytes.
const RF64_THRESHOLD: u64 = u32::MAX as u64 - 1024;

pub struct WavWriter<W: Write + Seek> {
    w: BufWriter<W>,
    channels: u16,
    data_offset: u64, // position of the first audio byte
    fact_offset: u64, // position of the fact chunk's frame count
    data_bytes: u64,
    rf64_threshold: u64,
}

impl<W: Write + Seek> WavWriter<W> {
    pub fn new(inner: W, sample_rate: u32, channels: u16) -> io::Result<Self> {
        Self::with_rf64_threshold(inner, sample_rate, channels, RF64_THRESHOLD)
    }

    /// Lower threshold exists so tests can exercise RF64 without 4 GB of data.
    pub fn with_rf64_threshold(
        inner: W,
        sample_rate: u32,
        channels: u16,
        rf64_threshold: u64,
    ) -> io::Result<Self> {
        if channels == 0 || sample_rate == 0 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "channels and rate must be > 0"));
        }
        let mut w = BufWriter::with_capacity(1 << 20, inner);
        let extensible = channels > 2;
        let block_align = channels as u32 * 4;

        w.write_all(b"RIFF")?;
        w.write_all(&0u32.to_le_bytes())?; // patched later
        w.write_all(b"WAVE")?;
        w.write_all(b"JUNK")?;
        w.write_all(&JUNK_LEN.to_le_bytes())?;
        w.write_all(&[0u8; JUNK_LEN as usize])?;

        w.write_all(b"fmt ")?;
        w.write_all(&(if extensible { 40u32 } else { 18u32 }).to_le_bytes())?;
        w.write_all(&(if extensible { 0xFFFEu16 } else { 3u16 }).to_le_bytes())?;
        w.write_all(&channels.to_le_bytes())?;
        w.write_all(&sample_rate.to_le_bytes())?;
        w.write_all(&(sample_rate * block_align).to_le_bytes())?;
        w.write_all(&(block_align as u16).to_le_bytes())?;
        w.write_all(&32u16.to_le_bytes())?;
        w.write_all(&(if extensible { 22u16 } else { 0u16 }).to_le_bytes())?;
        if extensible {
            w.write_all(&32u16.to_le_bytes())?; // valid bits
            w.write_all(&0u32.to_le_bytes())?; // channel mask: unspecified
            // KSDATAFORMAT_SUBTYPE_IEEE_FLOAT
            w.write_all(&[
                0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0xAA, 0x00, 0x38,
                0x9B, 0x71,
            ])?;
        }

        w.write_all(b"fact")?;
        w.write_all(&4u32.to_le_bytes())?;
        let fact_offset = w.stream_position()?;
        w.write_all(&0u32.to_le_bytes())?;

        w.write_all(b"data")?;
        w.write_all(&0u32.to_le_bytes())?; // patched later
        let data_offset = w.stream_position()?;

        let mut me = Self { w, channels, data_offset, fact_offset, data_bytes: 0, rf64_threshold };
        me.flush_header()?; // valid (empty) file from the start
        Ok(me)
    }

    pub fn frames_written(&self) -> u64 {
        self.data_bytes / (self.channels as u64 * 4)
    }

    /// Appends interleaved samples; the length must be a whole number of frames.
    pub fn write_samples(&mut self, samples: &[f32]) -> io::Result<()> {
        if samples.len() % self.channels as usize != 0 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "partial frame"));
        }
        for s in samples {
            self.w.write_all(&s.to_le_bytes())?;
        }
        self.data_bytes += samples.len() as u64 * 4;
        Ok(())
    }

    /// Flushes buffered audio and rewrites every size field so the file on
    /// disk is valid as of now.
    pub fn flush_header(&mut self) -> io::Result<()> {
        self.w.flush()?;
        let frames = self.frames_written();
        let file_len = self.data_offset + self.data_bytes;
        let riff_payload = file_len - 8;
        let rf64 = riff_payload > self.rf64_threshold;

        // Header patching seeks around; always come back to the end afterwards.
        self.w.seek(SeekFrom::Start(0))?;
        if rf64 {
            self.w.write_all(b"RF64")?;
            self.w.write_all(&u32::MAX.to_le_bytes())?;
            self.w.seek(SeekFrom::Start(12))?;
            self.w.write_all(b"ds64")?;
            self.w.write_all(&JUNK_LEN.to_le_bytes())?;
            self.w.write_all(&riff_payload.to_le_bytes())?;
            self.w.write_all(&self.data_bytes.to_le_bytes())?;
            self.w.write_all(&frames.to_le_bytes())?;
            self.w.write_all(&0u32.to_le_bytes())?; // table length
            self.w.seek(SeekFrom::Start(self.fact_offset))?;
            self.w.write_all(&u32::MAX.to_le_bytes())?;
            self.w.seek(SeekFrom::Start(self.data_offset - 4))?;
            self.w.write_all(&u32::MAX.to_le_bytes())?;
        } else {
            self.w.write_all(b"RIFF")?;
            self.w.write_all(&(riff_payload as u32).to_le_bytes())?;
            self.w.seek(SeekFrom::Start(self.fact_offset))?;
            self.w.write_all(&(frames as u32).to_le_bytes())?;
            self.w.seek(SeekFrom::Start(self.data_offset - 4))?;
            self.w.write_all(&(self.data_bytes as u32).to_le_bytes())?;
        }
        self.w.seek(SeekFrom::Start(file_len))?;
        self.w.flush()
    }

    /// Final header patch; returns the underlying writer.
    pub fn finish(mut self) -> io::Result<W> {
        self.flush_header()?;
        self.w.into_inner().map_err(|e| e.into_error())
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn ramp(frames: usize, channels: usize) -> Vec<f32> {
        (0..frames * channels).map(|i| (i as f32) * 0.001 - 0.5).collect()
    }

    fn read_back(bytes: Vec<u8>) -> (hound::WavSpec, Vec<f32>) {
        let mut r = hound::WavReader::new(Cursor::new(bytes)).unwrap();
        let spec = r.spec();
        (spec, r.samples::<f32>().map(|s| s.unwrap()).collect())
    }

    #[test]
    fn stereo_roundtrip_with_independent_reader() {
        let mut w = WavWriter::new(Cursor::new(Vec::new()), 48_000, 2).unwrap();
        let data = ramp(1000, 2);
        w.write_samples(&data).unwrap();
        let bytes = w.finish().unwrap().into_inner();
        let (spec, got) = read_back(bytes);
        assert_eq!((spec.channels, spec.sample_rate, spec.bits_per_sample), (2, 48_000, 32));
        assert_eq!(spec.sample_format, hound::SampleFormat::Float);
        assert_eq!(got, data);
    }

    #[test]
    fn six_channel_extensible_roundtrip() {
        let mut w = WavWriter::new(Cursor::new(Vec::new()), 96_000, 6).unwrap();
        let data = ramp(500, 6);
        w.write_samples(&data).unwrap();
        let (spec, got) = read_back(w.finish().unwrap().into_inner());
        assert_eq!((spec.channels, spec.sample_rate), (6, 96_000));
        assert_eq!(got, data);
    }

    #[test]
    fn file_is_valid_after_header_flush_without_finish() {
        // Simulates a crash: audio written after the last flush_header() is
        // lost, everything before it is readable.
        let mut w = WavWriter::new(Cursor::new(Vec::new()), 48_000, 2).unwrap();
        let first = ramp(300, 2);
        w.write_samples(&first).unwrap();
        w.flush_header().unwrap();
        w.write_samples(&ramp(50, 2)).unwrap();
        w.w.flush().unwrap(); // bytes reach the file but the header is stale
        let bytes = w.w.get_ref().get_ref().clone();
        let (_, got) = read_back(bytes);
        assert_eq!(got, first);
    }

    #[test]
    fn partial_frames_are_rejected() {
        let mut w = WavWriter::new(Cursor::new(Vec::new()), 48_000, 2).unwrap();
        assert!(w.write_samples(&[0.0; 3]).is_err());
    }

    #[test]
    fn rf64_conversion_when_threshold_exceeded() {
        let mut w = WavWriter::with_rf64_threshold(Cursor::new(Vec::new()), 48_000, 2, 1000).unwrap();
        let data = ramp(400, 2); // 3200 bytes > 1000
        w.write_samples(&data).unwrap();
        let bytes = w.finish().unwrap().into_inner();
        assert_eq!(&bytes[0..4], b"RF64");
        assert_eq!(&bytes[4..8], &u32::MAX.to_le_bytes());
        assert_eq!(&bytes[12..16], b"ds64");
        let riff = u64::from_le_bytes(bytes[20..28].try_into().unwrap());
        let data_size = u64::from_le_bytes(bytes[28..36].try_into().unwrap());
        let frames = u64::from_le_bytes(bytes[36..44].try_into().unwrap());
        assert_eq!(riff, bytes.len() as u64 - 8);
        assert_eq!(data_size, 3200);
        assert_eq!(frames, 400);
        // data chunk size field is the RF64 "look in ds64" marker
        let data_hdr = bytes.len() - 3200 - 4;
        assert_eq!(&bytes[data_hdr - 4..data_hdr], b"data");
        assert_eq!(&bytes[data_hdr..data_hdr + 4], &u32::MAX.to_le_bytes());
    }

    #[test]
    fn below_threshold_stays_riff() {
        let mut w = WavWriter::new(Cursor::new(Vec::new()), 48_000, 2).unwrap();
        w.write_samples(&ramp(10, 2)).unwrap();
        let bytes = w.finish().unwrap().into_inner();
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[12..16], b"JUNK");
    }
}
