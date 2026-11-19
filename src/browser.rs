//! Mode web: daftar tab dan normalisasi alamat. Bagian ini tidak menyentuh
//! WebView supaya bisa diuji murni; `Tabs<T>` menyimpan WebView sebagai `T`.

use serde::Serialize;

/// Tinggi chrome (tab bar 40 + toolbar 52) dalam piksel logis.
pub const CHROME_HEIGHT: f64 = 92.0;

pub struct Tab<T> {
    pub id: u32,
    pub title: String,
    pub url: String,
    pub view: T,
}

pub struct Tabs<T> {
    items: Vec<Tab<T>>,
    active: Option<u32>,
    next_id: u32,
}

impl<T> Default for Tabs<T> {
    fn default() -> Self {
        Self { items: Vec::new(), active: None, next_id: 1 }
    }
}

impl<T> Tabs<T> {
    pub fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Tambah tab dengan id dari `alloc_id` dan jadikan aktif.
    pub fn push(&mut self, id: u32, url: &str, view: T) {
        self.items.push(Tab { id, title: String::new(), url: url.to_string(), view });
        self.active = Some(id);
    }

    /// Tutup tab; aktif berpindah ke tetangga terdekat bila yang aktif ditutup.
    pub fn close(&mut self, id: u32) -> Option<Tab<T>> {
        let pos = self.items.iter().position(|t| t.id == id)?;
        let tab = self.items.remove(pos);
        if self.active == Some(id) {
            let next = pos.min(self.items.len().saturating_sub(1));
            self.active = self.items.get(next).map(|t| t.id);
        }
        Some(tab)
    }

    pub fn select(&mut self, id: u32) -> bool {
        let ok = self.items.iter().any(|t| t.id == id);
        if ok {
            self.active = Some(id);
        }
        ok
    }

    pub fn active_id(&self) -> Option<u32> {
        self.active
    }

    pub fn active(&self) -> Option<&Tab<T>> {
        self.items.iter().find(|t| Some(t.id) == self.active)
    }

    pub fn get_mut(&mut self, id: u32) -> Option<&mut Tab<T>> {
        self.items.iter_mut().find(|t| t.id == id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Tab<T>> {
        self.items.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn info(&self) -> Vec<TabInfo> {
        self.items
            .iter()
            .map(|t| TabInfo {
                id: t.id,
                title: t.title.clone(),
                url: t.url.clone(),
                active: Some(t.id) == self.active,
            })
            .collect()
    }
}

/// Data polos tab untuk `GET /api/tabs`.
#[derive(Serialize, Clone, Debug)]
pub struct TabInfo {
    pub id: u32,
    pub title: String,
    pub url: String,
    pub active: bool,
}

pub type SharedTabs = std::sync::Arc<std::sync::Mutex<Vec<TabInfo>>>;

/// Ubah isi address bar menjadi URL: skema utuh → apa adanya, mirip host →
/// `https://`, selain itu → pencarian DuckDuckGo.
pub fn normalize(input: &str) -> String {
    let s = input.trim();
    if s.starts_with("http://") || s.starts_with("https://") || s.starts_with("about:") {
        return s.to_string();
    }
    let host = s.split(['/', '?', '#']).next().unwrap_or("");
    let hostname = host.rsplit_once(':').map_or(host, |(h, p)| {
        if !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()) { h } else { host }
    });
    let looks_host = !s.contains(char::is_whitespace)
        && !hostname.is_empty()
        && (hostname == "localhost" || hostname.contains('.'))
        && !hostname.starts_with('.')
        && !hostname.ends_with('.');
    if looks_host {
        return format!("https://{s}");
    }
    let q: String = s
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            b' ' => "+".to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect();
    format!("https://duckduckgo.com/?q={q}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_cases() {
        assert_eq!(normalize("https://a.com/x"), "https://a.com/x");
        assert_eq!(normalize("google.com"), "https://google.com");
        assert_eq!(normalize("192.168.1.1:8080"), "https://192.168.1.1:8080");
        assert_eq!(normalize("localhost:3000/a"), "https://localhost:3000/a");
        assert_eq!(normalize("cara belajar"), "https://duckduckgo.com/?q=cara+belajar");
        assert_eq!(normalize("c++"), "https://duckduckgo.com/?q=c%2B%2B");
        assert_eq!(normalize("kata"), "https://duckduckgo.com/?q=kata");
    }

    #[test]
    fn tab_order_and_active() {
        let mut t: Tabs<()> = Tabs::default();
        let ids: Vec<u32> = (0..3).map(|_| t.alloc_id()).collect();
        assert_eq!(ids, [1, 2, 3]);
        for &i in &ids {
            t.push(i, "u", ());
        }
        assert_eq!(t.active_id(), Some(3));
        assert!(t.select(1));
        assert!(!t.select(99));
        t.close(1);
        assert_eq!(t.active_id(), Some(2)); // tetangga yang menggantikan posisi
        t.close(3);
        assert_eq!(t.active_id(), Some(2));
        t.close(2);
        assert!(t.is_empty() && t.active_id().is_none());
        assert_eq!(t.alloc_id(), 4); // id tidak dipakai ulang
    }
}
