//! Preset storage: one JSON file per preset in the data directory.
//! A preset is the full set of settable mixer parameters (never meters).

use std::fs;
use std::path::{Path, PathBuf};

use audiora_core::MixerParams;
use serde_json::{json, Map, Value};

/// Name used for the automatic save on exit; hidden from `list`.
pub const LAST: &str = "_last";
/// Same for the channel layout (Game/Chat/Media/Aux/Music/Mic), whose strip numbers mean
/// something else.
pub const LAST_CHANNELS: &str = "_last_ch2";
/// The channel session from before the Music channel: strip 4 was the microphone, 5 the tone.
pub const LEGACY_CHANNELS: &str = "_last_ch";

fn reserved(name: &str) -> bool {
    name == LAST || name == LAST_CHANNELS || name == LEGACY_CHANNELS
}

/// Moves a setting from the old five-channel layout to the six-channel one: Music took strip 4,
/// so the microphone moved 4 -> 5 and the test tone 5 -> 6. The new Music strip starts fresh.
pub fn legacy_channel_path(path: &str) -> String {
    for (from, to) in [("strip/5/", "strip/6/"), ("strip/4/", "strip/5/")] {
        if let Some(rest) = path.strip_prefix(from) {
            return format!("{to}{rest}");
        }
    }
    path.to_owned()
}

pub struct PresetStore {
    dir: PathBuf,
}

pub fn default_data_dir() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(|p| PathBuf::from(p).join("Roujan").join("Audiora"))
        .unwrap_or_else(|| std::env::temp_dir().join("Audiora"))
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && !name.starts_with('.')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '_' | '-'))
}

impl PresetStore {
    pub fn new(data_dir: &Path) -> Self {
        Self { dir: data_dir.join("presets") }
    }

    fn path(&self, name: &str) -> Result<PathBuf, String> {
        if !reserved(name) && !valid_name(name) {
            return Err("preset names may use letters, digits, space, - and _ (max 64)".into());
        }
        Ok(self.dir.join(format!("{name}.json")))
    }

    pub fn save(&self, name: &str, params: &MixerParams) -> Result<usize, String> {
        let path = self.path(name)?;
        fs::create_dir_all(&self.dir).map_err(|e| format!("preset folder: {e}"))?;
        let settings = params.settings();
        let map: Map<String, Value> = settings.iter().map(|(k, v)| (k.clone(), json!(v))).collect();
        let body = json!({ "version": 1, "name": name, "params": map });
        // Write to a temp file first so a crash never leaves a half-written preset.
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_vec_pretty(&body).unwrap()).map_err(|e| e.to_string())?;
        fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
        Ok(settings.len())
    }

    /// Applies a preset; returns (applied, skipped) where skipped paths do not
    /// exist in this engine layout (for example a different number of strips).
    pub fn load(&self, name: &str, params: &MixerParams) -> Result<(usize, usize), String> {
        self.load_mapped(name, params, str::to_owned)
    }

    /// Like `load`, with each stored path translated first (layout migrations).
    pub fn load_mapped(&self, name: &str, params: &MixerParams, map_path: impl Fn(&str) -> String) -> Result<(usize, usize), String> {
        let path = self.path(name)?;
        let text = fs::read_to_string(&path).map_err(|_| format!("no preset named \"{name}\""))?;
        let v: Value = serde_json::from_str(&text).map_err(|e| format!("corrupt preset: {e}"))?;
        let map = v.get("params").and_then(Value::as_object).ok_or("corrupt preset: no params")?;
        let (mut applied, mut skipped) = (0, 0);
        for (k, val) in map {
            match val.as_f64().map(|n| params.set(&map_path(k), n)) {
                Some(Ok(())) => applied += 1,
                _ => skipped += 1,
            }
        }
        Ok((applied, skipped))
    }

    pub fn delete(&self, name: &str) -> Result<(), String> {
        let path = self.path(name)?;
        fs::remove_file(path).map_err(|_| format!("no preset named \"{name}\""))
    }

    pub fn exists(&self, name: &str) -> bool {
        self.path(name).map(|p| p.exists()).unwrap_or(false)
    }

    pub fn list(&self) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(&self.dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let p = e.path();
                (p.extension()? == "json").then(|| p.file_stem()?.to_str().map(str::to_owned))?
            })
            .filter(|n| !reserved(n))
            .collect();
        names.sort_by_key(|n| n.to_lowercase());
        names
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (PresetStore, PathBuf) {
        let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("audiora-presets-{}-{n}", std::process::id()));
        (PresetStore::new(&dir), dir)
    }

    #[test]
    fn save_load_roundtrip_and_listing() {
        let (s, dir) = store();
        let a = MixerParams::new(2, 1);
        a.set("strip/0/gain_db", -7.5).unwrap();
        a.set("strip/1/comp/enabled", 1.0).unwrap();
        s.save("Streaming", &a).unwrap();
        s.save("alpha", &a).unwrap();
        s.save(LAST, &a).unwrap();
        assert_eq!(s.list(), vec!["alpha", "Streaming"]);

        let b = MixerParams::new(2, 1);
        let (applied, skipped) = s.load("Streaming", &b).unwrap();
        assert_eq!(skipped, 0);
        assert!(applied > 30);
        assert_eq!(b.get("strip/0/gain_db").unwrap(), -7.5);
        assert_eq!(b.get("strip/1/comp/enabled").unwrap(), 1.0);

        s.delete("alpha").unwrap();
        assert!(s.load("alpha", &b).is_err());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn preset_from_bigger_layout_skips_missing_paths() {
        let (s, dir) = store();
        let big = MixerParams::new(3, 2);
        s.save("big", &big).unwrap();
        let small = MixerParams::new(1, 1);
        let (applied, skipped) = s.load("big", &small).unwrap();
        assert!(applied > 0 && skipped > 0);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn rejects_path_traversal_and_bad_names() {
        let (s, _dir) = store();
        let p = MixerParams::new(1, 1);
        for bad in ["../evil", "a/b", "a\\b", "", ".hidden", "x:y", &"n".repeat(65)] {
            assert!(s.save(bad, &p).is_err(), "{bad:?}");
            assert!(s.load(bad, &p).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn the_old_channel_session_keeps_the_microphone_on_the_microphone() {
        let (s, dir) = store();
        // old layout: 4 app strips, mic = strip 4, tone = strip 5
        let old = MixerParams::new(6, 1);
        old.set("strip/4/boost_db", 12.0).unwrap();
        old.set("strip/4/denoise", 1.0).unwrap();
        old.set("strip/2/gain_db", -6.0).unwrap();
        s.save(LEGACY_CHANNELS, &old).unwrap();
        // new layout: Music is strip 4, the mic strip 5, the tone strip 6
        let new = MixerParams::new(7, 1);
        s.load_mapped(LEGACY_CHANNELS, &new, legacy_channel_path).unwrap();
        assert_eq!(new.get("strip/5/boost_db").unwrap(), 12.0);
        assert_eq!(new.get("strip/5/denoise").unwrap(), 1.0);
        assert_eq!(new.get("strip/2/gain_db").unwrap(), -6.0, "Media stays where it was");
        assert_eq!(new.get("strip/4/boost_db").unwrap(), 0.0, "Music starts fresh");
        assert_eq!(new.get("strip/4/denoise").unwrap(), 0.0);
        std::fs::remove_dir_all(dir).ok();
    }
}
