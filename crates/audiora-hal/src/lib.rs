//! Hardware abstraction layer. Today: WASAPI shared mode. Planned: WASAPI
//! exclusive, the Audiora ASIO driver and the R Interface native backend,
//! all behind the same device/stream vocabulary defined here.

#[cfg(windows)]
mod wasapi;

#[cfg(windows)]
pub use wasapi::{default_device_id, list_devices, CaptureStream, RenderStream};

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

#[derive(Clone, Copy, Debug)]
pub struct StreamFormat {
    pub sample_rate: u32,
    pub channels: u16,
    /// Frames the device buffer can hold; callbacks never exceed this.
    pub buffer_frames: u32,
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
