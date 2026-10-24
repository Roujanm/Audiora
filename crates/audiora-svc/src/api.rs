//! Control API: one JSON object per line in, one JSON object per line out.
//!
//!   {"id":1,"cmd":"ping"}
//!   {"cmd":"devices"}
//!   {"cmd":"info"}
//!   {"cmd":"state"}
//!   {"cmd":"get","path":"strip/0/gain_db"}
//!   {"cmd":"set","path":"strip/0/eq/1/gain_db","value":3.5}
//!   {"cmd":"meters"}
//!   {"cmd":"preset_list"} / preset_save / preset_load / preset_delete  ("name")
//!   {"cmd":"rec_start","sources":["mix"]} / rec_stop / rec_status
//!
//! Replies carry `"ok":true` or `"ok":false,"error":"..."`; `id` is echoed.

use std::sync::atomic::Ordering::Relaxed;
use std::sync::{Arc, Mutex};

use audiora_core::MixerParams;
use audiora_device::{ControlKind, HardwareInterface};
use serde_json::{json, Map, Value};

use crate::engine::Stats;
use crate::presets::{PresetStore, LAST};
use crate::rec::{self, RecController};

pub const API_VERSION: u32 = 1;

pub struct Api {
    pub params: Arc<MixerParams>,
    pub stats: Arc<Stats>,
    pub rec: Arc<RecController>,
    pub presets: Arc<PresetStore>,
    /// Attached interface hardware (or the labelled simulator), if any.
    pub hw: Option<Mutex<Box<dyn HardwareInterface>>>,
    /// Where `license.json` lives (the data directory).
    pub data_dir: std::path::PathBuf,
    /// Static engine description returned by `info`.
    pub info: Value,
}

impl Api {
    pub fn handle(&self, line: &str) -> String {
        let req: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => return err_reply(&Value::Null, &format!("invalid JSON: {e}")),
        };
        let id = req.get("id").cloned().unwrap_or(Value::Null);
        let Some(cmd) = req.get("cmd").and_then(Value::as_str) else {
            return err_reply(&id, "missing \"cmd\"");
        };
        let path = req.get("path").and_then(Value::as_str);

        let body = match cmd {
            "ping" => Ok(json!({ "api": API_VERSION })),
            "info" => Ok(self.info.clone()),
            "devices" => self.devices(),
            "state" => Ok(self.state()),
            "meters" => Ok(self.meters_json()),
            "license_status" => Ok(self.license_status()),
            "hw_info" => self.hw_info(),
            "hw_controls" => self.hw_controls(),
            "hw_meters" => self.hw_with(|hw| {
                let m: Map<String, Value> = hw.meters().into_iter().map(|(k, v)| (k, json!(v))).collect();
                Ok(json!({ "meters": m }))
            }),
            "hw_get" => match path {
                Some(p) => self.hw_with(|hw| {
                    hw.get(p).map(|v| json!({ "path": p, "value": v })).map_err(|e| e.to_string())
                }),
                None => Err("missing \"path\"".into()),
            },
            "hw_set" => match (path, req.get("value").and_then(as_number)) {
                (Some(p), Some(v)) => self.hw_with(|hw| {
                    hw.set(p, v).and_then(|_| hw.get(p)).map(|v| json!({ "path": p, "value": v })).map_err(|e| e.to_string())
                }),
                _ => Err("\"hw_set\" needs \"path\" and a numeric/boolean \"value\"".into()),
            },
            "preset_list" => Ok(json!({ "presets": self.presets.list() })),
            "preset_save" => self.with_name(&req, |n| {
                self.presets.save(n, &self.params).map(|c| json!({ "name": n, "saved": c }))
            }),
            "preset_load" => self.with_name(&req, |n| {
                self.presets
                    .load(n, &self.params)
                    .map(|(a, s)| json!({ "name": n, "applied": a, "skipped": s }))
            }),
            "preset_delete" => self.with_name(&req, |n| {
                self.presets.delete(n).map(|_| json!({ "name": n }))
            }),
            "rec_start" => {
                let sources: Vec<String> = req
                    .get("sources")
                    .and_then(Value::as_array)
                    .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect())
                    .unwrap_or_default();
                self.rec.start(&sources)
            }
            "rec_stop" => self.rec.stop().map(|s| rec::summary_json(&s)),
            "rec_status" => Ok(self.rec.status()),
            "get" => match path {
                Some(p) => self
                    .params
                    .get(p)
                    .map(|v| json!({ "path": p, "value": v }))
                    .map_err(|e| e.to_string()),
                None => Err("missing \"path\"".into()),
            },
            "set" => match (path, req.get("value").and_then(as_number)) {
                (Some(p), Some(v)) => self
                    .params
                    .set(p, v)
                    .and_then(|_| self.params.get(p))
                    .map(|v| json!({ "path": p, "value": v }))
                    .map_err(|e| e.to_string()),
                _ => Err("\"set\" needs \"path\" and a numeric/boolean \"value\"".into()),
            },
            other => Err(format!("unknown command \"{other}\"")),
        };

        match body {
            Ok(mut v) => {
                if let Some(o) = v.as_object_mut() {
                    o.insert("ok".into(), json!(true));
                    o.insert("id".into(), id);
                }
                v.to_string()
            }
            Err(e) => err_reply(&id, &e),
        }
    }

    /// Verifies `license.json` in the data directory against the vendor key
    /// baked into the build. Development builds have no key and say so.
    pub fn license_status(&self) -> Value {
        let path = self.data_dir.join("license.json");
        let Some(public) = audiora_license::embedded_public_key() else {
            return json!({ "state": "dev-build", "detail": "no vendor key embedded; license checks are off" });
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            return json!({ "state": "none", "edition": "play", "path": path.display().to_string() });
        };
        let serials: Vec<String> = self
            .hw
            .as_ref()
            .and_then(|h| h.lock().ok().map(|h| h.info().serial))
            .into_iter()
            .collect();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        match audiora_license::verify(&text, &public, now, &serials) {
            Ok(c) => json!({
                "state": "valid", "edition": c.edition, "issued_to": c.issued_to,
                "expires_at": c.expires_at, "device_serial": c.device_serial, "features": c.features,
            }),
            Err(e) => json!({ "state": "invalid", "edition": "play", "error": e.to_string() }),
        }
    }

    fn hw_with(&self, f: impl FnOnce(&mut dyn HardwareInterface) -> Result<Value, String>) -> Result<Value, String> {
        let hw = self.hw.as_ref().ok_or("no interface hardware connected")?;
        let mut guard = hw.lock().map_err(|_| "hardware lock poisoned".to_string())?;
        f(guard.as_mut())
    }

    pub fn hw_info(&self) -> Result<Value, String> {
        self.hw_with(|hw| {
            let i = hw.info();
            Ok(json!({
                "model": i.model, "serial": i.serial, "firmware": i.firmware,
                "simulated": i.simulated, "inputs": i.inputs, "outputs": i.outputs,
            }))
        })
    }

    fn hw_controls(&self) -> Result<Value, String> {
        self.hw_with(|hw| {
            let controls: Vec<Value> = hw
                .controls()
                .into_iter()
                .map(|c| {
                    let kind = match c.kind {
                        ControlKind::Range { min, max, step, unit } => {
                            json!({ "type": "range", "min": min, "max": max, "step": step, "unit": unit })
                        }
                        ControlKind::Toggle => json!({ "type": "toggle" }),
                        ControlKind::Choice { options } => json!({ "type": "choice", "options": options }),
                    };
                    let value = hw.get(&c.path).ok();
                    json!({ "path": c.path, "label": c.label, "group": c.group, "kind": kind, "value": value })
                })
                .collect();
            Ok(json!({ "controls": controls }))
        })
    }

    fn with_name(
        &self,
        req: &Value,
        f: impl FnOnce(&str) -> Result<Value, String>,
    ) -> Result<Value, String> {
        match req.get("name").and_then(Value::as_str) {
            Some(n) if n != LAST => f(n),
            Some(_) => Err("that name is reserved".into()),
            None => Err("missing \"name\"".into()),
        }
    }

    /// Paths of every meter, in the order `meter_values` reports them.
    pub fn meter_paths(&self) -> Vec<String> {
        self.params.paths().into_iter().filter(|p| self.params.is_meter(p)).collect()
    }

    pub fn meter_values(&self, paths: &[String]) -> Vec<f64> {
        paths.iter().map(|p| self.params.get(p).unwrap_or(0.0)).collect()
    }

    fn meters_json(&self) -> Value {
        let paths = self.meter_paths();
        let values = self.meter_values(&paths);
        json!({ "paths": paths, "values": values })
    }

    fn state(&self) -> Value {
        let params: Map<String, Value> =
            self.params.snapshot().into_iter().map(|(k, v)| (k, json!(v))).collect();
        json!({
            "params": params,
            "stats": {
                "callbacks": self.stats.callbacks.load(Relaxed),
                "underruns": self.stats.underruns.load(Relaxed),
                "drift_drops": self.stats.drift_drops.load(Relaxed),
                "capture_drops": self.stats.capture_drops.load(Relaxed),
            }
        })
    }

    fn devices(&self) -> Result<Value, String> {
        let list = audiora_hal::list_devices().map_err(|e| e.to_string())?;
        let devices: Vec<Value> = list
            .into_iter()
            .map(|d| {
                json!({
                    "id": d.id,
                    "name": d.name,
                    "flow": if d.flow == audiora_hal::Flow::Render { "render" } else { "capture" },
                    "default": d.is_default,
                    "sample_rate": d.sample_rate,
                    "channels": d.channels,
                })
            })
            .collect();
        Ok(json!({ "devices": devices }))
    }
}

fn as_number(v: &Value) -> Option<f64> {
    match v {
        Value::Bool(b) => Some(*b as u8 as f64),
        other => other.as_f64(),
    }
}

fn err_reply(id: &Value, msg: &str) -> String {
    json!({ "ok": false, "id": id, "error": msg }).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use audiora_core::Mixer;

    fn api() -> Api {
        let (_m, params) = Mixer::new(48_000.0, 2, 1);
        let dir = std::env::temp_dir().join(format!("audiora-api-test-{}", std::process::id()));
        let rec = Arc::new(RecController::new(Default::default(), 48_000, dir, crate::rec::Layout::new(2, 1, 1)));
        let presets = Arc::new(PresetStore::new(&std::env::temp_dir().join(format!("audiora-api-presets-{}", std::process::id()))));
        let hw: Option<Mutex<Box<dyn HardwareInterface>>> = Some(Mutex::new(Box::new(audiora_device::SimulatedR::new())));
        Api { params, stats: Arc::new(Stats::default()), rec, presets, hw, data_dir: std::env::temp_dir(), info: json!({}) }
    }

    fn call(a: &Api, line: &str) -> Value {
        serde_json::from_str(&a.handle(line)).unwrap()
    }

    #[test]
    fn set_then_get_roundtrip_with_id_echo() {
        let a = api();
        let r = call(&a, r#"{"id":7,"cmd":"set","path":"strip/0/gain_db","value":-9.5}"#);
        assert_eq!(r["ok"], true);
        assert_eq!(r["id"], 7);
        let r = call(&a, r#"{"cmd":"get","path":"strip/0/gain_db"}"#);
        assert!((r["value"].as_f64().unwrap() + 9.5).abs() < 1e-6);
    }

    #[test]
    fn booleans_and_clamping() {
        let a = api();
        assert_eq!(call(&a, r#"{"cmd":"set","path":"strip/1/mute","value":true}"#)["value"], 1.0);
        assert_eq!(call(&a, r#"{"cmd":"set","path":"bus/0/gain_db","value":500}"#)["value"], 24.0);
    }

    #[test]
    fn errors_are_reported_not_panicked() {
        let a = api();
        assert_eq!(call(&a, "not json")["ok"], false);
        assert_eq!(call(&a, r#"{"cmd":"nope"}"#)["ok"], false);
        assert_eq!(call(&a, r#"{"cmd":"set","path":"strip/9/gain_db","value":1}"#)["ok"], false);
        assert_eq!(call(&a, r#"{"cmd":"set","path":"strip/0/peak_l","value":1}"#)["ok"], false);
        assert_eq!(call(&a, r#"{"cmd":"set","path":"strip/0/gain_db"}"#)["ok"], false);
    }

    #[test]
    fn recording_commands_validate_state() {
        let a = api();
        assert_eq!(call(&a, r#"{"cmd":"rec_status"}"#)["recording"], false);
        assert_eq!(call(&a, r#"{"cmd":"rec_stop"}"#)["ok"], false);
        let r = call(&a, r#"{"cmd":"rec_start","sources":["nope"]}"#);
        assert_eq!(r["ok"], false);
        assert!(r["error"].as_str().unwrap().contains("unknown source"));
    }

    #[test]
    fn presets_via_api() {
        let a = api();
        call(&a, r#"{"cmd":"set","path":"strip/0/gain_db","value":-12}"#);
        assert_eq!(call(&a, r#"{"cmd":"preset_save","name":"api test"}"#)["ok"], true);
        call(&a, r#"{"cmd":"set","path":"strip/0/gain_db","value":0}"#);
        let r = call(&a, r#"{"cmd":"preset_load","name":"api test"}"#);
        assert_eq!(r["skipped"], 0);
        assert_eq!(call(&a, r#"{"cmd":"get","path":"strip/0/gain_db"}"#)["value"], -12.0);
        assert!(call(&a, r#"{"cmd":"preset_list"}"#)["presets"].as_array().unwrap().iter().any(|p| p == "api test"));
        assert_eq!(call(&a, r#"{"cmd":"preset_delete","name":"api test"}"#)["ok"], true);
        assert_eq!(call(&a, r#"{"cmd":"preset_save","name":"_last"}"#)["ok"], false);
        assert_eq!(call(&a, r#"{"cmd":"preset_save","name":"../x"}"#)["ok"], false);
    }

    #[test]
    fn hardware_commands_roundtrip_and_validate() {
        let a = api();
        assert_eq!(call(&a, r#"{"cmd":"hw_info"}"#)["simulated"], true);
        let controls = call(&a, r#"{"cmd":"hw_controls"}"#);
        assert!(controls["controls"].as_array().unwrap().iter().any(|c| c["path"] == "input/0/phantom" && c["kind"]["type"] == "toggle"));
        assert_eq!(call(&a, r#"{"cmd":"hw_set","path":"input/0/gain_db","value":33}"#)["value"], 33.0);
        assert_eq!(call(&a, r#"{"cmd":"hw_set","path":"input/0/gain_db","value":900}"#)["ok"], false);
        assert_eq!(call(&a, r#"{"cmd":"hw_get","path":"bogus"}"#)["ok"], false);
        assert!(call(&a, r#"{"cmd":"hw_meters"}"#)["meters"]["input/0/peak"].is_number());
    }

    #[test]
    fn license_status_reports_dev_build_without_vendor_key() {
        let a = api();
        let r = call(&a, r#"{"cmd":"license_status"}"#);
        assert_eq!(r["ok"], true);
        if audiora_license::embedded_public_key().is_none() {
            assert_eq!(r["state"], "dev-build");
        }
    }

    #[test]
    fn hardware_commands_fail_cleanly_without_hardware() {
        let mut a = api();
        a.hw = None;
        let r = call(&a, r#"{"cmd":"hw_info"}"#);
        assert_eq!(r["ok"], false);
        assert!(r["error"].as_str().unwrap().contains("no interface hardware"));
    }

    #[test]
    fn meters_report_paths_and_values_in_step() {
        let a = api();
        let m = call(&a, r#"{"cmd":"meters"}"#);
        assert_eq!(m["paths"].as_array().unwrap().len(), m["values"].as_array().unwrap().len());
        assert!(m["paths"].as_array().unwrap().iter().any(|p| p == "bus/0/limiter/gr_db"));
        assert!(!m["paths"].as_array().unwrap().iter().any(|p| p == "strip/0/gain_db"));
    }

    #[test]
    fn state_contains_params_and_stats() {
        let a = api();
        let r = call(&a, r#"{"cmd":"state"}"#);
        assert!(r["params"]["strip/1/eq/3/q"].is_number());
        assert_eq!(r["stats"]["underruns"], 0);
    }
}
