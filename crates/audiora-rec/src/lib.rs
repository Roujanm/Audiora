//! Recorder. [`WavWriter`] is the file format layer; [`RecordSession`] adds a
//! background disk thread fed by a wait-free ring so the audio thread never
//! touches the filesystem.

mod recorder;
mod wav;

pub use recorder::{RecProducer, RecStatus, RecordSession, Summary};
pub use wav::WavWriter;
