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

const STRIP_LEAVES: [&str; 10] =
    ["gain_db", "balance", "mute", "solo", "bus_mask", "peak_l", "peak_r", "denoise", "boost_db", "denoise_mix"];
const LOWCUT_LEAVES: [&str; 2] = ["enabled", "freq"];
const DEESS_LEAVES: [&str; 3] = ["enabled", "threshold_db", "gr_db"];
const SMART_LEAVES: [&str; 3] = ["enabled", "target_db", "gain_db"];
const GATE_LEAVES: [&str; 7] =
    ["enabled", "threshold_db", "attack_ms", "release_ms", "range_db", "hold_ms", "open"];
const COMP_LEAVES: [&str; 8] =
    ["enabled", "threshold_db", "ratio", "attack_ms", "release_ms", "makeup_db", "knee_db", "gr_db"];
const EQ_LEAVES: [&str; 3] = ["freq", "gain_db", "q"];
const BUS_LEAVES: [&str; 4] = ["gain_db", "mute", "peak_l", "peak_r"];
const LIMITER_LEAVES: [&str; 4] = ["enabled", "ceiling_db", "release_ms", "gr_db"];

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
                    ["denoise"] => Ok(Target::Flag(&s.denoise)),
                    ["boost_db"] => Ok(Target::Float(&s.boost_db, 0.0, 12.0)),
                    ["denoise_mix"] => Ok(Target::Float(&s.denoise_mix, 0.0, 1.0)),
                    ["lowcut", name] => match *name {
                        "enabled" => Ok(Target::Flag(&s.lowcut.enabled)),
                        "freq" => Ok(Target::Float(&s.lowcut.freq, 20.0, 300.0)),
                        _ => Err(UnknownPath),
                    },
                    ["deess", name] => match *name {
                        "enabled" => Ok(Target::Flag(&s.deess.enabled)),
                        "threshold_db" => Ok(Target::Float(&s.deess.threshold_db, -60.0, 0.0)),
                        "gr_db" => Ok(Target::Meter(&s.deess.gr_db)),
                        _ => Err(UnknownPath),
                    },
                    ["limiter", name] => match *name {
                        "enabled" => Ok(Target::Flag(&s.limiter.enabled)),
                        "ceiling_db" => Ok(Target::Float(&s.limiter.ceiling_db, -30.0, 0.0)),
                        "release_ms" => Ok(Target::Float(&s.limiter.release_ms, 1.0, 2000.0)),
                        "gr_db" => Ok(Target::Meter(&s.limiter.gr_db)),
                        _ => Err(UnknownPath),
                    },
                    ["smart", name] => match *name {
                        "enabled" => Ok(Target::Flag(&s.smart.enabled)),
                        "target_db" => Ok(Target::Float(&s.smart.target_db, -40.0, -10.0)),
                        "gain_db" => Ok(Target::Meter(&s.smart.gain_db)),
                        _ => Err(UnknownPath),
                    },
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
                    ["gate", name] => match *name {
                        "enabled" => Ok(Target::Flag(&s.gate.enabled)),
                        "threshold_db" => Ok(Target::Float(&s.gate.threshold_db, -100.0, 0.0)),
                        "attack_ms" => Ok(Target::Float(&s.gate.attack_ms, 0.1, 500.0)),
                        "release_ms" => Ok(Target::Float(&s.gate.release_ms, 1.0, 5000.0)),
                        "range_db" => Ok(Target::Float(&s.gate.range_db, 0.0, 100.0)),
                        "hold_ms" => Ok(Target::Float(&s.gate.hold_ms, 0.0, 1000.0)),
                        "open" => Ok(Target::Meter(&s.gate.open)),
                        _ => Err(UnknownPath),
                    },
                    ["comp", name] => match *name {
                        "enabled" => Ok(Target::Flag(&s.comp.enabled)),
                        "threshold_db" => Ok(Target::Float(&s.comp.threshold_db, -60.0, 0.0)),
                        "ratio" => Ok(Target::Float(&s.comp.ratio, 1.0, 20.0)),
                        "attack_ms" => Ok(Target::Float(&s.comp.attack_ms, 0.1, 500.0)),
                        "release_ms" => Ok(Target::Float(&s.comp.release_ms, 1.0, 5000.0)),
                        "makeup_db" => Ok(Target::Float(&s.comp.makeup_db, 0.0, 24.0)),
                        "knee_db" => Ok(Target::Float(&s.comp.knee_db, 0.0, 24.0)),
                        "gr_db" => Ok(Target::Meter(&s.comp.gr_db)),
                        _ => Err(UnknownPath),
                    },
                    _ => Err(UnknownPath),
                }
            }
            ["bus", i, rest @ ..] => {
                let b = self.buses.get(idx(i)?).ok_or(UnknownPath)?;
                match rest {
                    ["gain_db"] => Ok(Target::Float(&b.gain_db, -120.0, 24.0)),
                    ["mute"] => Ok(Target::Flag(&b.mute)),
                    ["peak_l"] => Ok(Target::Meter(&b.peak_l)),
                    ["peak_r"] => Ok(Target::Meter(&b.peak_r)),
                    ["limiter", name] => match *name {
                        "enabled" => Ok(Target::Flag(&b.limiter.enabled)),
                        "ceiling_db" => Ok(Target::Float(&b.limiter.ceiling_db, -30.0, 0.0)),
                        "release_ms" => Ok(Target::Float(&b.limiter.release_ms, 1.0, 2000.0)),
                        "gr_db" => Ok(Target::Meter(&b.limiter.gr_db)),
                        _ => Err(UnknownPath),
                    },
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

    /// True for meters and other values that must not be stored in presets.
    pub fn is_meter(&self, path: &str) -> bool {
        matches!(self.resolve(path), Ok(Target::Meter(_)))
    }

    /// All parameter and meter paths, in a stable order.
    pub fn paths(&self) -> Vec<String> {
        let mut paths = Vec::new();
        for i in 0..self.strips.len() {
            for n in STRIP_LEAVES {
                paths.push(format!("strip/{i}/{n}"));
            }
            for n in GATE_LEAVES {
                paths.push(format!("strip/{i}/gate/{n}"));
            }
            for b in 0..EQ_BANDS {
                for n in EQ_LEAVES {
                    paths.push(format!("strip/{i}/eq/{b}/{n}"));
                }
            }
            for n in SMART_LEAVES {
                paths.push(format!("strip/{i}/smart/{n}"));
            }
            for n in COMP_LEAVES {
                paths.push(format!("strip/{i}/comp/{n}"));
            }
            for n in LOWCUT_LEAVES {
                paths.push(format!("strip/{i}/lowcut/{n}"));
            }
            for n in DEESS_LEAVES {
                paths.push(format!("strip/{i}/deess/{n}"));
            }
            for n in LIMITER_LEAVES {
                paths.push(format!("strip/{i}/limiter/{n}"));
            }
        }
        for i in 0..self.buses.len() {
            for n in BUS_LEAVES {
                paths.push(format!("bus/{i}/{n}"));
            }
            for n in LIMITER_LEAVES {
                paths.push(format!("bus/{i}/limiter/{n}"));
            }
        }
        paths
    }

    /// Every parameter and meter as (path, value), for UI sync.
    pub fn snapshot(&self) -> Vec<(String, f64)> {
        self.paths().into_iter().filter_map(|p| self.get(&p).ok().map(|v| (p, v))).collect()
    }

    /// Only settable parameters (no meters): what a preset stores.
    pub fn settings(&self) -> Vec<(String, f64)> {
        self.snapshot().into_iter().filter(|(p, _)| !self.is_meter(p)).collect()
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
        p.set("strip/0/comp/ratio", 4.0).unwrap();
        assert_eq!(p.get("strip/0/comp/ratio").unwrap(), 4.0);
        p.set("bus/0/limiter/ceiling_db", 5.0).unwrap();
        assert_eq!(p.get("bus/0/limiter/ceiling_db").unwrap(), 0.0);
        assert_eq!(p.set("strip/5/gain_db", 0.0), Err(ParamError::UnknownPath));
        assert_eq!(p.set("strip/0/nope", 0.0), Err(ParamError::UnknownPath));
        assert_eq!(p.set("strip/0/peak_l", 0.0), Err(ParamError::ReadOnly));
        assert_eq!(p.set("strip/0/comp/gr_db", 0.0), Err(ParamError::ReadOnly));
        assert_eq!(p.set("strip/0/gain_db", f64::NAN), Err(ParamError::BadValue));
    }

    #[test]
    fn every_listed_path_resolves() {
        let p = MixerParams::new(3, 2);
        let paths = p.paths();
        assert_eq!(paths.len(), 3 * (10 + 7 + 18 + 3 + 8 + 2 + 3 + 4) + 2 * (4 + 4));
        for path in &paths {
            assert!(p.get(path).is_ok(), "{path}");
        }
        assert!(paths.iter().any(|k| k == "strip/1/eq/5/q"));
    }

    #[test]
    fn settings_exclude_meters_and_roundtrip() {
        let p = MixerParams::new(1, 1);
        assert!(p.is_meter("strip/0/peak_l") && p.is_meter("bus/0/limiter/gr_db"));
        let settings = p.settings();
        assert!(settings.iter().all(|(k, _)| !p.is_meter(k)));
        p.set("strip/0/gate/enabled", 1.0).unwrap();
        p.set("strip/0/gain_db", -3.0).unwrap();
        let saved = p.settings();
        let q = MixerParams::new(1, 1);
        for (k, v) in &saved {
            q.set(k, *v).unwrap();
        }
        assert_eq!(q.get("strip/0/gate/enabled").unwrap(), 1.0);
        assert_eq!(q.get("strip/0/gain_db").unwrap(), -3.0);
    }
}
