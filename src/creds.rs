//! Penyimpanan kredensial MyKlass dan mode sesi.
//!
//! Password tidak pernah ditulis sebagai plaintext. Di Windows, password
//! dienkripsi dengan DPAPI (`CryptProtectData`) memakai kunci milik user yang
//! sedang login, jadi file `creds.json` hanya bisa dibuka oleh akun Windows
//! yang sama di mesin yang sama.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::store;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Kiosk penuh: header SEB dikirim, anti-cheat dinetralkan, jendela terkunci.
    #[default]
    Seb,
    /// Browser biasa: tanpa header SEB, tanpa stub, jendela normal.
    Web,
}

impl Mode {
    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Seb => "seb",
            Mode::Web => "web",
        }
    }

    pub fn parse(raw: &str) -> Option<Mode> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "seb" => Some(Mode::Seb),
            "web" => Some(Mode::Web),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct Stored {
    #[serde(default)]
    mode: Option<Mode>,
    #[serde(default)]
    username: String,
    /// Password terenkripsi DPAPI, base64.
    #[serde(default)]
    password: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PublicCreds {
    pub mode: &'static str,
    pub username: String,
    pub has_password: bool,
}

fn creds_path() -> PathBuf {
    store::data_dir().join("creds.json")
}

fn read() -> Stored {
    match fs::read_to_string(creds_path()) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => Stored::default(),
    }
}

fn write(state: &Stored) -> std::io::Result<()> {
    let dir = store::data_dir();
    fs::create_dir_all(&dir)?;
    let path = creds_path();
    let tmp = path.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(state).unwrap_or_else(|_| "{}".into());
    fs::write(&tmp, text)?;
    fs::rename(&tmp, &path)
}

pub fn mode() -> Mode {
    read().mode.unwrap_or_default()
}

pub fn set_mode(mode: Mode) -> std::io::Result<()> {
    let mut state = read();
    state.mode = Some(mode);
    write(&state)
}

pub fn public() -> PublicCreds {
    let state = read();
    PublicCreds {
        mode: state.mode.unwrap_or_default().as_str(),
        username: state.username,
        has_password: !state.password.is_empty(),
    }
}

pub fn save(username: &str, password: &str) -> Result<(), String> {
    let username = username.trim();
    if username.is_empty() {
        return Err("Username kosong.".into());
    }
    let mut state = read();

    if password.is_empty() {
        // Password kosong = jangan sentuh yang tersimpan.
        state.username = username.to_string();
    } else {
        let sealed = protect(password.as_bytes()).map_err(|e| format!("Gagal mengenkripsi: {e}"))?;
        state.username = username.to_string();
        state.password = b64_encode(&sealed);
    }
    write(&state).map_err(|e| format!("Gagal menyimpan: {e}"))
}

pub fn clear_password() -> std::io::Result<()> {
    let mut state = read();
    state.password.clear();
    write(&state)
}

/// Username + password siap pakai, kalau ada.
pub fn credentials() -> Option<(String, String)> {
    let state = read();
    if state.username.is_empty() || state.password.is_empty() {
        return None;
    }
    let sealed = b64_decode(&state.password)?;
    let plain = unprotect(&sealed).ok()?;
    let password = String::from_utf8(plain).ok()?;
    Some((state.username, password))
}

/// Enkripsi teks ke base64 (DPAPI).
pub(crate) fn seal(plain: &str) -> Result<String, String> {
    protect(plain.as_bytes()).map(|b| b64_encode(&b))
}

/// Kebalikan `seal`.
pub(crate) fn unseal(sealed: &str) -> Option<String> {
    String::from_utf8(unprotect(&b64_decode(sealed)?).ok()?).ok()
}

// ---- base64 ----

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn b64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(B64[(n >> 18) as usize & 63] as char);
        out.push(B64[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { B64[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { B64[n as usize & 63] as char } else { '=' });
    }
    out
}

fn b64_decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let mut buf: u32 = 0;
    let mut bits = 0;
    for ch in text.bytes() {
        let val = match ch {
            b'A'..=b'Z' => ch - b'A',
            b'a'..=b'z' => ch - b'a' + 26,
            b'0'..=b'9' => ch - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            b'\r' | b'\n' | b' ' => continue,
            _ => return None,
        } as u32;
        buf = (buf << 6) | val;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Some(out)
}

// ---- DPAPI ----

#[cfg(target_os = "windows")]
fn protect(plain: &[u8]) -> Result<Vec<u8>, String> {
    use windows::Win32::Security::Cryptography::{
        CryptProtectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    let input = CRYPT_INTEGER_BLOB {
        cbData: plain.len() as u32,
        pbData: plain.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    unsafe {
        CryptProtectData(
            &input,
            windows::core::w!("WebTask"),
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
        .map_err(|e| e.to_string())?;
        let bytes = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        windows::Win32::Foundation::LocalFree(Some(windows::Win32::Foundation::HLOCAL(
            output.pbData as *mut _,
        )));
        Ok(bytes)
    }
}

#[cfg(target_os = "windows")]
fn unprotect(sealed: &[u8]) -> Result<Vec<u8>, String> {
    use windows::Win32::Security::Cryptography::{
        CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    let input = CRYPT_INTEGER_BLOB {
        cbData: sealed.len() as u32,
        pbData: sealed.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    unsafe {
        CryptUnprotectData(
            &input,
            None,
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
        .map_err(|e| e.to_string())?;
        let bytes = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        windows::Win32::Foundation::LocalFree(Some(windows::Win32::Foundation::HLOCAL(
            output.pbData as *mut _,
        )));
        Ok(bytes)
    }
}

#[cfg(not(target_os = "windows"))]
fn protect(_plain: &[u8]) -> Result<Vec<u8>, String> {
    Err("DPAPI hanya tersedia di Windows.".into())
}

#[cfg(not(target_os = "windows"))]
fn unprotect(_sealed: &[u8]) -> Result<Vec<u8>, String> {
    Err("DPAPI hanya tersedia di Windows.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_roundtrip() {
        for sample in [&b""[..], b"a", b"ab", b"abc", b"admin:rahasia"] {
            let enc = b64_encode(sample);
            assert_eq!(b64_decode(&enc).unwrap(), sample, "gagal untuk {sample:?}");
        }
    }

    #[test]
    fn base64_known() {
        assert_eq!(b64_encode(b"abc"), "YWJj");
        assert_eq!(b64_encode(b"ab"), "YWI=");
        assert_eq!(b64_encode(b"a"), "YQ==");
    }

    #[test]
    fn mode_parse() {
        assert_eq!(Mode::parse("SEB"), Some(Mode::Seb));
        assert_eq!(Mode::parse("web"), Some(Mode::Web));
        assert_eq!(Mode::parse("lain"), None);
    }
}