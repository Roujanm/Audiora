//! Hardware control layer for the R Interface.
//!
//! The wire protocol of the real device is not defined yet (see
//! `docs/R-INTERFACE.md`), so this crate defines what the rest of Audiora
//! needs from *any* interface — a capability description, parameters by path,
//! and meters — and ships [`SimulatedR`], a software stand-in used to build
//! and test the control UI. The real USB backend will implement
//! [`HardwareInterface`] and replace the simulator without touching callers.

mod sim;

pub use sim::SimulatedR;

use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub enum ControlKind {
    /// Continuous value in `min..=max` (`unit` is for display only, e.g. "dB").
    Range { min: f64, max: f64, step: f64, unit: &'static str },
    /// 0 or 1.
    Toggle,
    /// Index into `options`.
    Choice { options: Vec<String> },
}

#[derive(Clone, Debug, PartialEq)]
pub struct ControlDesc {
    /// Stable address, e.g. `input/0/phantom`.
    pub path: String,
    pub label: String,
    /// UI grouping, e.g. "Input 1".
    pub group: String,
    pub kind: ControlKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HwInfo {
    pub model: String,
    pub serial: String,
    pub firmware: String,
    /// True for the software simulator; UIs must show this.
    pub simulated: bool,
    pub inputs: usize,
    pub outputs: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub enum HwError {
    UnknownPath,
    OutOfRange,
    Disconnected,
}

impl fmt::Display for HwError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            HwError::UnknownPath => "unknown hardware control",
            HwError::OutOfRange => "value outside the control's range",
            HwError::Disconnected => "interface disconnected",
        })
    }
}

impl std::error::Error for HwError {}

pub trait HardwareInterface: Send {
    fn info(&self) -> HwInfo;
    fn controls(&self) -> Vec<ControlDesc>;
    fn get(&self, path: &str) -> Result<f64, HwError>;
    fn set(&mut self, path: &str, value: f64) -> Result<(), HwError>;
    /// Level meters as (path, linear peak), e.g. `input/0/peak`.
    fn meters(&mut self) -> Vec<(String, f64)>;
}
