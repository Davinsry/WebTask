# Rencana: Mode Web Seperti Chrome

Status: rencana, belum ada kode. Mode SEB tidak disentuh.

## Tujuan

Mode web biasa menjadi browser mandiri di dalam WebTask:

- Tab bar (buka, tutup, pindah tab).
- Toolbar: back, forward, reload, address bar.
- Ketik alamat atau kata pencarian di address bar.
- Setiap tab memuat situs apa pun seperti Chrome.

Di luar cakupan: sinkronisasi akun, ekstensi, profil, unduhan, bookmark, privasi
incognito, dan panel devtools buatan sendiri.

## Batasan teknis yang sudah diverifikasi

| Hal | Fakta |
| --- | --- |
| `tao` 0.37.2 | Ada `set_decorations`, `set_resizable`, `set_fullscreen`, `set_inner_size`, `set_outer_position`, `set_visible`. Tidak ada `set_skip_taskbar`. |
| `wry` 0.57.2 | Ada `build_as_child`, `set_bounds`, `set_visible`, `focus`, `load_url`, `evaluate_script`, `url()`. Tidak ada API back/forward langsung dan tidak ada callback resize otomatis untuk webview anak. |
| `wry` + `with_additional_browser_args` | Menimpa default wry; argumen default harus ditulis ulang. |
| WebView2 | Satu jendela boleh punya beberapa WebView anak (controller terpisah). |
| Riwayat | Back/forward hanya bisa lewat `evaluate_script("history.back()")` di webview tab tersebut. Tidak ada `canGoBack`. |

## Arsitektur

```
Jendela tao (satu)
├── WebView chrome  (root, memuat ui/browser.html)
│     tab bar + toolbar + address bar, tinggi tetap CHROME_HEIGHT = 92 px
│     (tab bar 40 px + toolbar 52 px)
└── WebView anak 1..n (satu per tab)
      bounds = client area, y mulai CHROME_HEIGHT
      hanya tab aktif yang visible
```

Alasan memakai banyak WebView anak, bukan iframe:

- Situs dengan `X-Frame-Options`/`frame-ancestors` (Google, GitHub, MyKlass)
  tidak bisa dimuat di iframe.
- Isolasi cookie dan sesi per tab seperti browser sungguhan.
- Toolbar tidak ikut ter-refresh saat halaman navigasi.

Alasan chrome berupa WebView HTML, bukan widget tao:

- Tab bar dinamis (judul berubah, tombol tutup per tab) jauh lebih murah di HTML.
- Konsisten dengan palet DESIGN.md dan gaya panel yang sudah ada.

## Struktur berkas

```
src/
  browser.rs        (baru) pengelola tab
  app.rs            mode web: chrome + tab, mode SEB: seperti sekarang
  server.rs         route /browser
  ui/
    shell.html      panel SEB dan panel info (tanpa tab bar)
    browser.html    (baru) chrome browser: tab bar, toolbar, address bar
    picker*.html    overlay pilih mode (sudah ada, di shell.html)
```

## Modul `browser.rs`

```rust
pub const CHROME_HEIGHT: f64 = 92.0;

pub struct Tab {
    pub id: u32,
    pub title: String,
    pub url: String,
    webview: wry::WebView,
}

pub struct Browser {
    tabs: Vec<Tab>,
    active: Option<u32>,
    next_id: u32,
}

impl Browser {
    pub fn open(&mut self, window: &tao::window::Window, url: &str) -> Result<u32, wry::Error>;
    pub fn close(&mut self, id: u32);
    pub fn select(&mut self, id: u32);
    pub fn active(&self) -> Option<&Tab>;
    pub fn navigate(&self, url: &str);
    pub fn back(&self);
    pub fn forward(&self);
    pub fn reload(&self);
    pub fn layout(&self, width: f64, height: f64);
    pub fn sync_titles(&mut self);
}
```

Catatan implementasi:

- `open`: `WebViewBuilder::new().with_url(url).with_ipc_handler(...).build_as_child(window)`,
  lalu `set_visible(false)`, push ke `tabs`, panggil `select(id)`.
- `layout`: satu `Rect` dipakai semua tab:
  `position = (0, CHROME_HEIGHT)`, `size = (width, height - CHROME_HEIGHT)`.
- `select`: set semua `set_visible(tab.id == id)`, lalu `focus()` tab aktif.
- Tab terakhir ditutup: tidak perlu menutup browser, cukup tampilkan halaman
  awal di chrome (tab baru otomatis dibuka saat chrome meminta).
- `with_initialization_script` per tab hanya untuk melaporkan judul:
  `document.title` dikirim via IPC saat load dan saat `titlechange`.

## IPC

Satu jenis pesan, `type: "tab"`, dengan `cmd`:

| `cmd` | Payload | Aksi Rust |
| --- | --- | --- |
| `new` | `url` (opsional, default `myklass...`) | `Browser::open` |
| `close` | `id` | `Browser::close` |
| `select` | `id` | `Browser::select` |
| `go` | `url` | `Browser::navigate` pada tab aktif |
| `back` | – | `history.back()` |
| `forward` | – | `history.forward()` |
| `reload` | – | `location.reload()` |

Arah sebaliknya (tab ke chrome), `type: "tab_state"`:

| Field | Isi |
| --- | --- |
| `id` | id tab |
| `title` | `document.title` |
| `url` | `location.href` |
| `active` | id tab aktif |

Chrome memakainya untuk memperbarui judul tab, isi address bar, dan status tombol.

## Event loop (`app.rs`)

Tambahan `UserEvent`:

```rust
enum UserEvent {
    // ... yang sudah ada
    PickMode(creds::Mode),
    Tab(TabCmd),
    TabState { id: u32, title: String, url: String },
}
```

Alur:

1. Startup: jendela kecil 440×300, muat shell, overlay pilih mode muncul.
2. Pilih SEB: seperti sekarang (kiosk, panel config).
3. Pilih web: ukuran 1280×860 di tengah layar, lalu muat `/browser`.
4. `Resized`: hitung ukuran logis, `browser.layout(w, h)`.
5. `Tab(TabCmd::*)`: jalankan di `Browser`.
6. `TabState`: update judul + address bar di chrome lewat `evaluate_script`
   (`window.__wtTabState(...)`), atau cukup biarkan chrome memintanya lewat
   `GET /api/tabs` (lebih sederhana, tanpa push).

Pilihan lebih sederhana untuk sinkronisasi: **polling** dari chrome setiap
600 ms ke `GET /api/tabs` yang mengembalikan daftar tab (id, title, url, active)
dari `Browser`. Menghindari dua arah IPC yang rawan balapan. Keputusan: pakai
endpoint polling ini.

Untuk judul tab, `Browser::sync_titles` memanggil `evaluate_script` yang
mengirim IPC `tab_title` sekali saat halaman selesai dimuat. Bila terlalu rumit,
judul diambil dari hostname URL-nya saja (cukup untuk kebutuhan saat ini).

## Server

Endpoint baru di `server.rs`:

| Metode | Path | Fungsi |
| --- | --- | --- |
| GET | `/browser` | menyajikan `ui/browser.html` |
| GET | `/api/tabs` | daftar tab + tab aktif |

`/api/tabs` mengambil data dari state bersama. Karena `Browser` tidak `Send`
(berisi `WebView`), daftar tab dicerminkan ke `Arc<Mutex<Vec<TabInfo>>>` yang
diperbarui setiap kali tab berubah. `TabInfo` hanya berisi data polos
(`id`, `title`, `url`, `active`).

## `ui/browser.html`

Struktur:

```html
<div class="tabs" role="tablist"> … </div>
<div class="bar">
  <button id="back">   <button id="fwd">   <button id="reload">
  <form id="addr"><input id="url"></form>
  <button id="newtab">+
  <button id="panel">  (kembali ke panel config / ganti mode)
</div>
```

Perilaku:

- Tinggi total tepat 92 px, `body` tidak bisa scroll.
- Tab aktif: `aria-selected="true"`, latar permukaan.
- Tombol tutup per tab, minimal satu tab selalu ada.
- Address bar menampilkan `location.href` tab aktif; saat fokus, teks terpilih.
- Enter di address bar:
  - Punya skema (`http://`, `https://`) → langsung.
  - Terlihat seperti host (`contoh.com`, `192.168.1.1:8080`) → awali `https://`.
  - Selain itu → `https://duckduckgo.com/?q=<encoded>`.
- Back/forward/reload mengirim `tab` ke tab aktif.
- Hint keyboard: `Ctrl+T` tab baru, `Ctrl+W` tutup tab, `Alt+Left/Right`
  back/forward, `Ctrl+L` fokus address bar.

## Pemisahan mode

- Mode SEB: tidak memakai `browser.html` sama sekali. Panel shell seperti
  sekarang, header config dan kiosk aktif.
- Mode web: chrome browser dimuat, **tidak ada** header SEB
  (`windows_header_hook` sudah memeriksa `creds::mode()`).
- Anti-cheat netral tetap disuntik di kedua mode lewat `init_script()` dan
  `ANTICHEAT_NEUTRALIZE_JS` pada setiap WebView tab.

## Titik rawan dan mitigasi

1. **WebView anak tidak ikut resize.** Mitigasi: hitung `layout` pada setiap
   `Event::WindowEvent::Resized` dan sekali setelah mode dipilih.
2. **Navigasi diabaikan WebView2 saat jendela sedang berubah style.** Ini sudah
   pernah terjadi pada implementasi picker halaman terpisah. Mitigasi:
   arsitektur overlay dipakai; chrome dan tab baru dibuat setelah mode dipilih
   dan jendela sudah stabil.
3. **Tab menutupi toolbar.** Mitigasi: `y` selalu `CHROME_HEIGHT`, tinggi tab
   dikurangi. Ada test manual: toolbar harus bisa diklik di tab mana pun.
4. **`evaluate_script` back/forward tidak akurat** bila halaman memakai SPA.
   Diterima; ini batas WebView2.
5. **Popup `window.open`** dari situs tidak membuka tab baru. Mitigasi tahap 1:
   abaikan. Tahap 2: `with_new_window_req_handler` → buka tab baru.
6. **Tab yatim saat jendela ditutup.** `Browser` di-drop bersama `WebView`
   anaknya; tidak ada resource yang bocor menurut dokumentasi wry.

## Urutan pengerjaan

1. `src/browser.rs` + unit test murni (urutan tab, id unik, perpindahan aktif)
   tanpa `WebView` (pakai trait kecil atau state terpisah).
2. `ui/browser.html` (tampilan dulu, tanpa IPC): tab bar statis + toolbar.
3. Integrasi `app.rs`: `UserEvent::PickMode` mode web memuat `/browser` dan
   membuat tab pertama.
4. IPC dua arah tab ↔ Rust.
5. `/api/tabs` + sinkronisasi judul.
6. Resize handler.
7. Keyboard shortcut.
8. Uji: Chrome-like manual (Google, YouTube, MyKlass, situs dengan
   `X-Frame-Options`), plus `cargo test` dan `clippy`.
9. Commit, pindahkan tag `v0.1.0`, verifikasi CI dan aset rilis.

## Definisi selesai

- Buka aplikasi → pilih web → jendela 1280×860 dengan tab bar dan toolbar.
- Ketik `google.com` → Google terbuka di area tab, toolbar tetap di atas.
- `Ctrl+T` membuka tab baru; `Ctrl+W` menutup tab; pindah tab mengganti halaman
  tanpa reload.
- Ketik `cara belajar` → hasil DuckDuckGo.
- Pilih SEB di startup → perilaku kiosk identik dengan versi sekarang.
- `cargo build --release`, `clippy`, dan `cargo test` bersih.