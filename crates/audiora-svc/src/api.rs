//! Control API: one JSON object per line in, one JSON object per line out.
//!
//!   {"id":1,"cmd":"ping"}
//!   {"cmd":"devices"}
//!   {"cmd":"info"}
//!   {"cmd":"state"}
//!   {"cmd":"get","path":"strip/0/gain_db"}
//!   {"cmd":"set","path":"strip/0/eq/1/gain_db","value":3.5}
//!
//! Replies carry `"ok":true` or `"ok":false,"error":"..."`; `id` is echoed.

use std::sync::atomic::Ordering::Relaxed;
use std::sync::Arc;

use audiora_core::MixerParams;
use serde_json::{json, Map, Value};

use crate::engine::Stats;
use crate::rec::{self, RecController};

pub const API_VERSION: u32 = 1;

pub struct Api {
    pub params: Arc<MixerParams>,
    pub stats: Arc<Stats>,
    pub rec: Arc<RecController>,
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
        Api { params, stats: Arc::new(Stats::default()), rec, info: json!({}) }
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
    fn state_contains_params_and_stats() {
        let a = api();
        let r = call(&a, r#"{"cmd":"state"}"#);
        assert!(r["params"]["strip/1/eq/3/q"].is_number());
        assert_eq!(r["stats"]["underruns"], 0);
    }
}
