//! Hardware abstraction layer. Today: WASAPI shared mode. Planned: WASAPI
//! exclusive, the Audiora ASIO driver and the R Interface native backend,
//! all behind the same device/stream vocabulary defined here.

mod format;
#[cfg(windows)]
mod sessions;
#[cfg(windows)]
mod wasapi;

#[cfg(windows)]
pub use sessions::{list_sessions, set_session, AppSession};
#[cfg(windows)]
pub use wasapi::{default_device_id, list_devices, CaptureStream, RenderStream};

pub use format::SampleFormat;

use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flow {
    Render,
    Capture,
}

#[derive(Clone, Debug)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub flow: Flow,
    pub is_default: bool,
    pub sample_rate: u32,
    pub channels: u16,
}

/// How a device stream is opened.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    /// Shared mode with the device's default period.
    #[default]
    Shared,
    /// Shared mode with the smallest period the audio engine offers (`IAudioClient3`).
    LowLatency,
    /// Exclusive mode: the application owns the device and talks to its native format.
    /// Lowest latency and bit-perfect, but no other application can use the device.
    Exclusive,
}

#[derive(Clone, Copy, Debug)]
pub struct StreamFormat {
    pub sample_rate: u32,
    pub channels: u16,
    /// Frames the device buffer can hold; callbacks never exceed this.
    pub buffer_frames: u32,
    /// Typical frames delivered per wake-up (the engine period).
    pub period_frames: u32,
    /// True when the IAudioClient3 minimum-period path is active.
    pub low_latency: bool,
    /// True when the device is opened in exclusive mode.
    pub exclusive: bool,
    /// Valid bits per sample on the wire (32 for float).
    pub bit_depth: u16,
}

#[derive(Debug)]
pub struct HalError(pub String);

impl fmt::Display for HalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for HalError {}

#[cfg(windows)]
impl From<windows::core::Error> for HalError {
    fn from(e: windows::core::Error) -> Self {
        HalError(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, HalError>;
