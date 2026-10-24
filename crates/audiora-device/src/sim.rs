//! Software stand-in for the R Interface: 2 combo inputs, 2 line outputs, a
//! headphone output and a direct-monitor mix. It behaves like a typical
//! 2-in/2-out interface so the control UI can be built against it. The meters
//! are synthetic (a slow wobble scaled by each input's gain) and say so.

use std::collections::BTreeMap;
use std::time::Instant;

use crate::{ControlDesc, ControlKind, HardwareInterface, HwError, HwInfo};

const INPUTS: usize = 2;

pub struct SimulatedR {
    values: BTreeMap<String, f64>,
    descs: Vec<ControlDesc>,
    started: Instant,
}

fn range(path: &str, label: &str, group: &str, min: f64, max: f64, step: f64, unit: &'static str) -> ControlDesc {
    ControlDesc {
        path: path.into(),
        label: label.into(),
        group: group.into(),
        kind: ControlKind::Range { min, max, step, unit },
    }
}

fn toggle(path: &str, label: &str, group: &str) -> ControlDesc {
    ControlDesc { path: path.into(), label: label.into(), group: group.into(), kind: ControlKind::Toggle }
}

impl Default for SimulatedR {
    fn default() -> Self {
        Self::new()
    }
}

impl SimulatedR {
    pub fn new() -> Self {
        let mut descs = Vec::new();
        let mut defaults: Vec<(String, f64)> = Vec::new();
        for i in 0..INPUTS {
            let g = format!("Input {}", i + 1);
            descs.push(range(&format!("input/{i}/gain_db"), "Gain", &g, 0.0, 60.0, 1.0, "dB"));
            descs.push(toggle(&format!("input/{i}/phantom"), "+48V", &g));
            descs.push(toggle(&format!("input/{i}/pad"), "Pad", &g));
            descs.push(ControlDesc {
                path: format!("input/{i}/source"),
                label: "Source".into(),
                group: g.clone(),
                kind: ControlKind::Choice { options: vec!["Mic".into(), "Line".into(), "Instrument".into()] },
            });
            descs.push(toggle(&format!("input/{i}/mute"), "Mute", &g));
            defaults.extend([
                (format!("input/{i}/gain_db"), 20.0),
                (format!("input/{i}/phantom"), 0.0),
                (format!("input/{i}/pad"), 0.0),
                (format!("input/{i}/source"), 0.0),
                (format!("input/{i}/mute"), 0.0),
            ]);
        }
        descs.push(range("output/main/volume_db", "Main volume", "Outputs", -60.0, 0.0, 0.5, "dB"));
        descs.push(toggle("output/main/mute", "Main mute", "Outputs"));
        descs.push(range("output/phones/volume_db", "Headphone volume", "Outputs", -60.0, 0.0, 0.5, "dB"));
        descs.push(range("monitor/mix", "Direct monitor mix (input ↔ playback)", "Monitoring", 0.0, 1.0, 0.01, ""));
        descs.push(toggle("monitor/mono", "Monitor mono", "Monitoring"));
        descs.push(ControlDesc {
            path: "clock/source".into(),
            label: "Clock source".into(),
            group: "Clock".into(),
            kind: ControlKind::Choice { options: vec!["Internal".into(), "S/PDIF".into()] },
        });
        defaults.extend([
            ("output/main/volume_db".into(), -6.0),
            ("output/main/mute".into(), 0.0),
            ("output/phones/volume_db".into(), -12.0),
            ("monitor/mix".into(), 0.0),
            ("monitor/mono".into(), 0.0),
            ("clock/source".into(), 0.0),
        ]);
        Self { values: defaults.into_iter().collect(), descs, started: Instant::now() }
    }

    fn desc(&self, path: &str) -> Option<&ControlDesc> {
        self.descs.iter().find(|d| d.path == path)
    }
}

impl HardwareInterface for SimulatedR {
    fn info(&self) -> HwInfo {
        HwInfo {
            model: "R Interface (SIMULATOR)".into(),
            serial: "SIM-000000".into(),
            firmware: "sim-0".into(),
            simulated: true,
            inputs: INPUTS,
            outputs: 2,
        }
    }

    fn controls(&self) -> Vec<ControlDesc> {
        self.descs.clone()
    }

    fn get(&self, path: &str) -> Result<f64, HwError> {
        self.values.get(path).copied().ok_or(HwError::UnknownPath)
    }

    fn set(&mut self, path: &str, value: f64) -> Result<(), HwError> {
        let valid = match &self.desc(path).ok_or(HwError::UnknownPath)?.kind {
            ControlKind::Range { min, max, .. } => value.is_finite() && value >= *min && value <= *max,
            ControlKind::Toggle => value == 0.0 || value == 1.0,
            ControlKind::Choice { options } => value.fract() == 0.0 && value >= 0.0 && (value as usize) < options.len(),
        };
        if !valid {
            return Err(HwError::OutOfRange);
        }
        // +48 V on a line/instrument source would be pointless and can damage some
        // gear; real hardware refuses it too, so the simulator does the same.
        if path.ends_with("/phantom") && value == 1.0 {
            let src = path.replace("/phantom", "/source");
            if self.values.get(&src).copied().unwrap_or(0.0) != 0.0 {
                return Err(HwError::OutOfRange);
            }
        }
        self.values.insert(path.to_string(), value);
        // Switching to a non-mic source drops phantom power.
        if path.ends_with("/source") && value != 0.0 {
            self.values.insert(path.replace("/source", "/phantom"), 0.0);
        }
        Ok(())
    }

    fn meters(&mut self) -> Vec<(String, f64)> {
        let t = self.started.elapsed().as_secs_f64();
        (0..INPUTS)
            .map(|i| {
                let gain = self.values[&format!("input/{i}/gain_db")];
                let muted = self.values[&format!("input/{i}/mute")] == 1.0;
                let wobble = 0.5 + 0.5 * (t * (1.3 + i as f64 * 0.7)).sin();
                let level = if muted { 0.0 } else { 0.002 * wobble * 10f64.powf(gain / 20.0) };
                (format!("input/{i}/peak"), level.min(1.0))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifies_itself_as_a_simulator() {
        let hw = SimulatedR::new();
        let i = hw.info();
        assert!(i.simulated && i.model.contains("SIMULATOR"));
    }

    #[test]
    fn every_described_control_has_a_value_and_default_is_valid() {
        let hw = SimulatedR::new();
        for d in hw.controls() {
            let v = hw.get(&d.path).unwrap_or_else(|_| panic!("{} has no value", d.path));
            match d.kind {
                ControlKind::Range { min, max, .. } => assert!((min..=max).contains(&v), "{}", d.path),
                ControlKind::Toggle => assert!(v == 0.0 || v == 1.0),
                ControlKind::Choice { ref options } => assert!((v as usize) < options.len()),
            }
        }
    }

    #[test]
    fn rejects_unknown_paths_and_out_of_range_values() {
        let mut hw = SimulatedR::new();
        assert_eq!(hw.set("nope", 1.0), Err(HwError::UnknownPath));
        assert_eq!(hw.set("input/0/gain_db", 99.0), Err(HwError::OutOfRange));
        assert_eq!(hw.set("input/0/gain_db", f64::NAN), Err(HwError::OutOfRange));
        assert_eq!(hw.set("input/0/phantom", 0.5), Err(HwError::OutOfRange));
        assert_eq!(hw.set("input/0/source", 3.0), Err(HwError::OutOfRange));
        hw.set("input/0/gain_db", 35.0).unwrap();
        assert_eq!(hw.get("input/0/gain_db"), Ok(35.0));
    }

    #[test]
    fn phantom_power_is_only_allowed_on_mic_source() {
        let mut hw = SimulatedR::new();
        hw.set("input/1/phantom", 1.0).unwrap();
        assert_eq!(hw.get("input/1/phantom"), Ok(1.0));
        // moving to an instrument input must switch +48 V off
        hw.set("input/1/source", 2.0).unwrap();
        assert_eq!(hw.get("input/1/phantom"), Ok(0.0));
        assert_eq!(hw.set("input/1/phantom", 1.0), Err(HwError::OutOfRange));
    }

    #[test]
    fn meters_follow_gain_and_mute() {
        let mut hw = SimulatedR::new();
        let loud = |hw: &mut SimulatedR| {
            (0..50).map(|_| { std::thread::sleep(std::time::Duration::from_millis(2)); hw.meters()[0].1 }).fold(0.0, f64::max)
        };
        hw.set("input/0/gain_db", 10.0).unwrap();
        let low = loud(&mut hw);
        hw.set("input/0/gain_db", 40.0).unwrap();
        let high = loud(&mut hw);
        assert!(high > low * 10.0);
        hw.set("input/0/mute", 1.0).unwrap();
        assert_eq!(hw.meters()[0].1, 0.0);
    }
}
