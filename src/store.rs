//! Penyimpanan config WebTask (multi-config, ~/.webtask/configs.json).

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub id: String,
    pub filename: String,
    #[serde(rename = "startURL")]
    pub start_url: String,
    pub host: String,
    pub cmid: Option<i64>,
    #[serde(rename = "configKey")]
    pub config_key: String,
    #[serde(rename = "importedAt")]
    pub imported_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct State {
    #[serde(default)]
    pub configs: Vec<Config>,
    #[serde(default)]
    pub active: Option<String>,
}

pub fn data_dir() -> PathBuf {
    dirs_home().join(".webtask")
}

fn store_path() -> PathBuf {
    data_dir().join("configs.json")
}

fn dirs_home() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn load() -> State {
    match fs::read_to_string(store_path()) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => State::default(),
    }
}

fn save(state: &State) -> std::io::Result<()> {
    fs::create_dir_all(data_dir())?;
    let path = store_path();
    let tmp = path.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(state).unwrap_or_else(|_| "{}".into());
    fs::write(&tmp, text)?;
    fs::rename(&tmp, &path)
}

fn timestamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format_timestamp(secs)
}

fn format_timestamp(secs: u64) -> String {
    let days = (secs / 86400) as i64;
    let rem = secs % 86400;
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let (y, mo, d) = civil_from_days(days);
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{m:02}:{s:02}")
}

// Konversi hari-sejak-1970 ke (tahun, bulan, hari), algoritma Howard Hinnant.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Tambah config (replace kalau cmid+host sama). Kembalikan entry.
pub fn add(meta: crate::keys::ConfigMeta) -> Config {
    let mut state = load();
    let entry = Config {
        id: uuid::Uuid::new_v4().to_string()[..12].to_string(),
        filename: meta.filename,
        start_url: meta.start_url,
        host: meta.host,
        cmid: meta.cmid,
        config_key: meta.config_key,
        imported_at: timestamp(),
    };
    state
        .configs
        .retain(|c| !(c.cmid == entry.cmid && c.host == entry.host));
    state.configs.insert(0, entry.clone());
    state.active = Some(entry.id.clone());
    let _ = save(&state);
    entry
}

pub fn activate(id: &str) -> Option<Config> {
    let mut state = load();
    if let Some(cfg) = state.configs.iter().find(|c| c.id == id).cloned() {
        state.active = Some(id.to_string());
        let _ = save(&state);
        Some(cfg)
    } else {
        None
    }
}

pub fn delete(id: &str) {
    let mut state = load();
    state.configs.retain(|c| c.id != id);
    if state.active.as_deref() == Some(id) {
        state.active = state.configs.first().map(|c| c.id.clone());
    }
    let _ = save(&state);
}

pub fn active() -> Option<Config> {
    let state = load();
    match state.active {
        Some(id) => state.configs.into_iter().find(|c| c.id == id),
        None => None,
    }
}

/// Config yang paling cocok untuk cmid tertentu (fallback: config aktif).
pub fn for_cmid(cmid: Option<i64>) -> Option<Config> {
    if let Some(cmid) = cmid {
        if let Some(cfg) = load().configs.into_iter().find(|c| c.cmid == Some(cmid)) {
            return Some(cfg);
        }
    }
    active()
}

pub fn all_configs() -> Vec<Config> {
    load().configs
}