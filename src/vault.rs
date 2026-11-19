//! Penyimpanan login per situs untuk mode web (seperti pengelola sandi browser).
//!
//! Password dienkripsi DPAPI (lihat `creds`), jadi `vault.json` hanya terbuka
//! untuk akun Windows yang sama. Host SELALU diambil dari URL tab oleh Rust,
//! bukan dari klaim halaman, supaya situs lain tidak bisa meminta sandi situs X.

use serde::{Deserialize, Serialize};
use std::fs;

use crate::{creds, store};

#[derive(Serialize, Deserialize, Default)]
struct Entry {
    host: String,
    username: String,
    /// Password terenkripsi DPAPI, base64.
    password: String,
}

#[derive(Serialize, Deserialize, Default)]
struct Vault {
    #[serde(default)]
    entries: Vec<Entry>,
    /// Situs yang dipilih "jangan simpan".
    #[serde(default)]
    never: Vec<String>,
}

fn path() -> std::path::PathBuf {
    store::data_dir().join("vault.json")
}

fn read() -> Vault {
    fs::read_to_string(path())
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn write(v: &Vault) -> Result<(), String> {
    let dir = store::data_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let tmp = path().with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_string_pretty(v).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    fs::rename(&tmp, path()).map_err(|e| e.to_string())
}

/// Host dari URL halaman. Hanya https (atau http ke localhost) yang dipakai,
/// supaya sandi tidak diisikan ke koneksi tanpa enkripsi.
pub fn host_of(url: &str) -> Option<String> {
    let (scheme, rest) = url.split_once("://")?;
    let authority = rest.split(['/', '?', '#']).next()?;
    let host = authority.rsplit('@').next()?;
    let host = host.rsplit_once(':').map_or(host, |(h, p)| {
        if p.bytes().all(|b| b.is_ascii_digit()) { h } else { host }
    });
    let host = host.to_ascii_lowercase();
    let local = host == "localhost" || host == "127.0.0.1";
    match scheme {
        "https" if !host.is_empty() => Some(host),
        "http" if local => Some(host),
        _ => None,
    }
}

pub fn is_never(host: &str) -> bool {
    read().never.iter().any(|h| h == host)
}

pub fn set_never(host: &str) {
    let mut v = read();
    if !v.never.iter().any(|h| h == host) {
        v.never.push(host.to_string());
        let _ = write(&v);
    }
}

/// Login tersimpan untuk host ini, yang terakhir diperbarui lebih dulu.
pub fn lookup(host: &str) -> Vec<(String, String)> {
    read()
        .entries
        .iter()
        .rev()
        .filter(|e| e.host == host)
        .filter_map(|e| {
            let plain = creds::unseal(&e.password)?;
            Some((e.username.clone(), plain))
        })
        .collect()
}

/// Simpan atau perbarui (host + username).
pub fn save(host: &str, username: &str, password: &str) -> Result<(), String> {
    let sealed = creds::seal(password)?;
    let mut v = read();
    v.entries.retain(|e| !(e.host == host && e.username == username));
    v.entries.push(Entry {
        host: host.to_string(),
        username: username.to_string(),
        password: sealed,
    });
    write(&v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_rules() {
        assert_eq!(host_of("https://Accounts.Google.com/x?y=1").as_deref(), Some("accounts.google.com"));
        assert_eq!(host_of("https://a.com:8443/").as_deref(), Some("a.com"));
        assert_eq!(host_of("http://localhost:3000/a").as_deref(), Some("localhost"));
        assert_eq!(host_of("http://example.com/"), None);
        assert_eq!(host_of("https://evil.com@good.com/").as_deref(), Some("good.com"));
        assert_eq!(host_of("about:blank"), None);
    }
}
