//! Parsing file `.seb` dan perhitungan Config Key.
//!
//! Replika persis `quizaccess_seb\config_key::generate()` + `property_list::to_json()`:
//! parse plist -> buang originatorVersion -> sort key case-insensitive ->
//! json compact (tanpa escape slash/unicode) -> sha256 hex

use serde_json::Value;
use sha2::{Digest, Sha256};

#[derive(Debug)]
pub struct ConfigError(pub String);

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ConfigError {}

fn err<T>(msg: impl Into<String>) -> Result<T, ConfigError> {
    Err(ConfigError(msg.into()))
}

#[derive(Debug)]
pub struct Seb {
    pub data: plist::Dictionary,
    pub start_url: String,
}

pub fn parse_seb(raw: &[u8]) -> Result<Seb, ConfigError> {
    if raw.is_empty() {
        return err("File kosong.");
    }
    if raw.len() > 1024 * 1024 {
        return err("File lebih dari 1 MB, tidak wajar untuk config SEB.");
    }
    if raw.starts_with(b"bplist") {
        return err("Config dalam format binary/terenkripsi. Minta versi unencrypted (.seb XML) ke dosen.");
    }
    let head = &raw[..raw.len().min(2000)];
    if !contains(head, b"<plist") {
        return err("Bukan file .seb (plist XML) yang valid.");
    }
    let value = plist::Value::from_reader(std::io::Cursor::new(raw))
        .map_err(|e| ConfigError(format!("Gagal parse plist: {e}. Kalau config dienkripsi password, minta versi unencrypted.")))?;
    let data = match value {
        plist::Value::Dictionary(d) => d,
        _ => return err("Root plist bukan dictionary."),
    };
    let start_url = match data.get("startURL").and_then(|v| v.as_string()) {
        Some(s) if s.starts_with("https://") => s.to_string(),
        _ => return err("Config tidak punya startURL https yang valid."),
    };
    Ok(Seb { data, start_url })
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

pub fn config_key_from_data(data: &plist::Dictionary) -> String {
    let mut entries: Vec<(String, Value)> = data
        .iter()
        .filter(|(k, _)| k.as_str() != "originatorVersion")
        .map(|(k, v)| (k.clone(), plist_to_json(v)))
        .collect();
    entries.sort_by(|a, b| {
        a.0.to_lowercase()
            .cmp(&b.0.to_lowercase())
            .then_with(|| a.0.cmp(&b.0))
    });

    let mut map = serde_json::Map::new();
    for (k, v) in entries {
        map.insert(k, v);
    }
    let json = serde_json::to_string(&Value::Object(map)).unwrap_or_default();
    sha256_hex(json.as_bytes())
}

fn plist_to_json(v: &plist::Value) -> Value {
    match v {
        plist::Value::String(s) => Value::String(s.clone()),
        plist::Value::Boolean(b) => Value::Bool(*b),
        plist::Value::Integer(i) => integer_to_json(*i),
        plist::Value::Real(r) => json_number(*r),
        plist::Value::Date(d) => Value::String(d.to_xml_format()),
        plist::Value::Data(bytes) => Value::String(base64_encode(bytes)),
        plist::Value::Array(items) => Value::Array(items.iter().map(plist_to_json).collect()),
        plist::Value::Dictionary(d) => {
            let mut map = serde_json::Map::new();
            for (k, v) in d.iter() {
                map.insert(k.clone(), plist_to_json(v));
            }
            Value::Object(map)
        }
        _ => Value::Null,
    }
}

fn integer_to_json(i: plist::Integer) -> Value {
    if let Some(v) = i.as_signed() {
        Value::Number(v.into())
    } else if let Some(v) = i.as_unsigned() {
        Value::Number(v.into())
    } else {
        Value::Null
    }
}

fn json_number(r: f64) -> Value {
    let n = serde_json::Number::from_f64(r);
    match n {
        Some(n) => Value::Number(n),
        None => Value::Null,
    }
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        out.push(TABLE[(b[0] >> 2) as usize] as char);
        out.push(TABLE[(((b[0] & 0b11) << 4) | (b[1] >> 4)) as usize] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(((b[1] & 0b1111) << 2) | (b[2] >> 6)) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[(b[2] & 0b111111) as usize] as char
        } else {
            '='
        });
    }
    out
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut out = String::with_capacity(64);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

pub fn parse_cmid(url: &str) -> Option<i64> {
    let query = url.split_once('?')?.1.split('#').next()?;
    for pair in query.split('&') {
        let (k, v) = pair.split_once('=')?;
        if (k == "id" || k == "cmid") && !v.is_empty() && v.bytes().all(|c| c.is_ascii_digit()) {
            return v.parse().ok();
        }
    }
    None
}

pub fn host_of(url: &str) -> String {
    let rest = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
    rest.split(['/', '?', '#']).next().unwrap_or("").to_string()
}

/// Nilai X-SafeExamBrowser-ConfigKeyHash untuk satu URL.
pub fn header_value(url: &str, config_key: &str) -> String {
    sha256_hex(format!("{url}{config_key}").as_bytes())
}

pub fn load_seb(raw: &[u8], filename: &str) -> Result<ConfigMeta, ConfigError> {
    let seb = parse_seb(raw)?;
    Ok(ConfigMeta {
        filename: filename.to_string(),
        start_url: seb.start_url.clone(),
        host: host_of(&seb.start_url),
        cmid: parse_cmid(&seb.start_url),
        config_key: config_key_from_data(&seb.data),
    })
}

#[derive(Debug, Clone)]
pub struct ConfigMeta {
    pub filename: String,
    pub start_url: String,
    pub host: String,
    pub cmid: Option<i64>,
    pub config_key: String,
}