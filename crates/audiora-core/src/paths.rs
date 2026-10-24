//! String-addressed access to every mixer parameter and meter, e.g.
//! `strip/0/eq/1/gain_db`. This is what the control API and presets use.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering::Relaxed};

use crate::params::{MixerParams, EQ_BANDS};
use crate::AtomicF32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamError {
    UnknownPath,
    ReadOnly,
    BadValue,
}

impl std::fmt::Display for ParamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ParamError::UnknownPath => "unknown parameter path",
            ParamError::ReadOnly => "parameter is read-only",
            ParamError::BadValue => "value is not a finite number",
        })
    }
}

impl std::error::Error for ParamError {}

enum Target<'a> {
    Float(&'a AtomicF32, f32, f32),
    Flag(&'a AtomicBool),
    Mask(&'a AtomicU32),
    Meter(&'a AtomicF32),
}

impl MixerParams {
    fn resolve(&self, path: &str) -> Result<Target<'_>, ParamError> {
        use ParamError::UnknownPath;
        let parts: Vec<&str> = path.split('/').collect();
        let idx = |s: &str| s.parse::<usize>().map_err(|_| UnknownPath);
        match parts.as_slice() {
            ["strip", i, rest @ ..] => {
                let s = self.strips.get(idx(i)?).ok_or(UnknownPath)?;
                match rest {
                    ["gain_db"] => Ok(Target::Float(&s.gain_db, -120.0, 24.0)),
                    ["balance"] => Ok(Target::Float(&s.balance, -1.0, 1.0)),
                    ["mute"] => Ok(Target::Flag(&s.mute)),
                    ["solo"] => Ok(Target::Flag(&s.solo)),
                    ["bus_mask"] => Ok(Target::Mask(&s.bus_mask)),
                    ["peak_l"] => Ok(Target::Meter(&s.peak_l)),
                    ["peak_r"] => Ok(Target::Meter(&s.peak_r)),
                    ["eq", b, name] => {
                        let band = s.eq.get(idx(b)?).ok_or(UnknownPath)?;
                        match *name {
                            "freq" => Ok(Target::Float(&band.freq, 20.0, 20_000.0)),
                            "gain_db" => Ok(Target::Float(&band.gain_db, -24.0, 24.0)),
                            "q" => Ok(Target::Float(&band.q, 0.1, 18.0)),
                            _ => Err(UnknownPath),
                        }
                    }
                    _ => Err(UnknownPath),
                }
            }
            ["bus", i, name] => {
                let b = self.buses.get(idx(i)?).ok_or(UnknownPath)?;
                match *name {
                    "gain_db" => Ok(Target::Float(&b.gain_db, -120.0, 24.0)),
                    "mute" => Ok(Target::Flag(&b.mute)),
                    "peak_l" => Ok(Target::Meter(&b.peak_l)),
                    "peak_r" => Ok(Target::Meter(&b.peak_r)),
                    _ => Err(UnknownPath),
                }
            }
            _ => Err(UnknownPath),
        }
    }

    /// Sets a parameter by path; out-of-range values are clamped.
    pub fn set(&self, path: &str, value: f64) -> Result<(), ParamError> {
        if !value.is_finite() {
            return Err(ParamError::BadValue);
        }
        match self.resolve(path)? {
            Target::Float(a, lo, hi) => a.set((value as f32).clamp(lo, hi)),
            Target::Flag(a) => a.store(value >= 0.5, Relaxed),
            Target::Mask(a) => a.store(value.max(0.0) as u32, Relaxed),
            Target::Meter(_) => return Err(ParamError::ReadOnly),
        }
        Ok(())
    }

    pub fn get(&self, path: &str) -> Result<f64, ParamError> {
        Ok(match self.resolve(path)? {
            Target::Float(a, ..) | Target::Meter(a) => a.get() as f64,
            Target::Flag(a) => a.load(Relaxed) as u8 as f64,
            Target::Mask(a) => a.load(Relaxed) as f64,
        })
    }

    /// Every parameter and meter as (path, value), for UI sync and presets.
    pub fn snapshot(&self) -> Vec<(String, f64)> {
        let mut paths = Vec::new();
        for i in 0..self.strips.len() {
            for n in ["gain_db", "balance", "mute", "solo", "bus_mask", "peak_l", "peak_r"] {
                paths.push(format!("strip/{i}/{n}"));
            }
            for b in 0..EQ_BANDS {
                for n in ["freq", "gain_db", "q"] {
                    paths.push(format!("strip/{i}/eq/{b}/{n}"));
                }
            }
        }
        for i in 0..self.buses.len() {
            for n in ["gain_db", "mute", "peak_l", "peak_r"] {
                paths.push(format!("bus/{i}/{n}"));
            }
        }
        paths.into_iter().filter_map(|p| self.get(&p).ok().map(|v| (p, v))).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_get_clamp_and_errors() {
        let p = MixerParams::new(2, 1);
        p.set("strip/1/gain_db", -6.0).unwrap();
        assert!((p.get("strip/1/gain_db").unwrap() + 6.0).abs() < 1e-6);
        p.set("strip/0/gain_db", 999.0).unwrap();
        assert_eq!(p.get("strip/0/gain_db").unwrap(), 24.0);
        p.set("strip/0/eq/2/freq", 3000.0).unwrap();
        assert_eq!(p.get("strip/0/eq/2/freq").unwrap(), 3000.0);
        p.set("bus/0/mute", 1.0).unwrap();
        assert_eq!(p.get("bus/0/mute").unwrap(), 1.0);
        assert_eq!(p.set("strip/5/gain_db", 0.0), Err(ParamError::UnknownPath));
        assert_eq!(p.set("strip/0/nope", 0.0), Err(ParamError::UnknownPath));
        assert_eq!(p.set("strip/0/peak_l", 0.0), Err(ParamError::ReadOnly));
        assert_eq!(p.set("strip/0/gain_db", f64::NAN), Err(ParamError::BadValue));
    }

    #[test]
    fn snapshot_lists_everything_readable() {
        let p = MixerParams::new(2, 2);
        let snap = p.snapshot();
        assert_eq!(snap.len(), 2 * (7 + 12) + 2 * 4);
        assert!(snap.iter().any(|(k, _)| k == "strip/1/eq/3/q"));
    }
}
