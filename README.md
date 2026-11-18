<img src="assets/logo.svg" width="64" height="64" alt="WebTask">

# WebTask

Ruang ujian tanpa kunci.

WebTask adalah browser kecil untuk quiz MyKlass UMY yang dikunci plugin Moodle
`quizaccess_seb` (Safe Exam Browser). Tarik file `.seb` yang dibagikan dosen,
WebTask hitung config key-nya dan langsung membuka quiz. Setelah itu semuanya
browser biasa: copy paste, screenshot, devtools.

Dulu ditulis dalam Python + Playwright. Sekarang **Rust + WebView2** supaya
jendelanya ringan (~beberapa MB, memakai WebView2 yang sudah ada di Windows 11)
dan tidak perlu membawa Chromium sendiri.

## Kenapa ini bisa jalan

Plugin `quizaccess_seb` memvalidasi tiga hal, dan hanya satu yang benar-benar menahan:

| Pemeriksaan | Aktif saat | Bisa dipenuhi? |
|---|---|---|
| User-Agent mengandung `SEB` | hanya mode `USE_SEB_CLIENT_CONFIG` | tidak perlu di mode lain |
| **Config key** | `CONFIG_MANUALLY`, `TEMPLATE`, `UPLOAD_CONFIG` | ya, dihitung dari file `.seb` |
| Browser exam key | `UPLOAD_CONFIG`, `CLIENT_CONFIG` | otomatis lolos kalau daftar BEK kosong |

Config key bukan rahasia kuat, itu turunan file `.seb`:

```
parse plist
buang entry originatorVersion
sort semua key case-insensitive (SORT_STRING | SORT_FLAG_CASE)
json compact, tanpa escape slash/unicode
sha256 hex  ->  config key
```

Yang dikirim ke Moodle bukan key mentah, tapi hash per halaman
(`access_manager::check_key`):

```
X-SafeExamBrowser-ConfigKeyHash: sha256(url_halaman + config_key)
```

WebTask memasang header itu di setiap request dokumen ke host quiz, plus stub
`window.SafeExamBrowser` supaya AMD module `quizaccess_seb/validate_quiz_access`
di halaman attempt ikut lolos.

## Instalasi

Butuh Rust (toolchain **MSVC**, bukan GNU) dan WebView2 Runtime (sudah ada di
Windows 10/11).

```powershell
rustup default stable-x86_64-pc-windows-msvc
cargo build --release
```

`scripts/login.py` (opsional, untuk Cloudflare) tetap Python:

```bash
pip install -r scripts/requirements.txt   # botasaurus, pyvirtualdisplay
```

## Pakai

```powershell
# 1. (opsional) login sekali kalau MyKlass di balik Cloudflare
python scripts/login.py

# 2. jalankan WebTask
cargo run --release
```

Jendela WebTask menampilkan panel. Tarik file `.seb` ke situ (atau klik
"Pilih file .seb"), quiz langsung terbuka di tab yang sama. Tombol `Panel`
di halaman quiz membawa kembali ke panel.

## Mode kiosk

Jendela berjalan kiosk: tanpa title bar, tanpa tombol minimize/maximize/close,
dan fullscreen borderless sehingga taskbar tertutup - hanya isi web yang
terlihat. Jendela tidak dipaksa selalu di atas, jadi aplikasi lain bisa
berpindah ke depan lewat Alt+Tab.

- **Alt+Tab** tetap bisa memindah ke aplikasi lain (jendela tetap muncul di
  taskbar dan daftar Alt+Tab).
- **Alt+F4** menutup WebTask.

## Yang disimpan

```
~/.webtask/configs.json     daftar config + config aktif
```

Format JSON kompatibel dengan versi Python lama, jadi config yang sudah ada
tetap terbaca.

Setiap quiz punya file `.seb` sendiri, jadi WebTask menyimpan banyak config
sekaligus dan memilih yang cocok **berdasarkan `cmid`**, bukan URL persis.
Config menyimpan `startURL = view.php?id=NNN` sementara pengerjaan terjadi di
`attempt.php?attempt=X&cmid=NNN`; matching by `cmid` itu wajib, kalau tidak
header tidak akan terpasang di halaman attempt.

## Verifikasi

WebTask memanggil WS `quizaccess_seb_validate_quiz_keys` setiap halaman quiz
selesai load, lalu menampilkan toast:

- hijau, `WebTask: config cocok` → `{"configkey": true, "browserexamkey": true}`
- merah, `WebTask: config tidak cocok...` → config untuk quiz ini salah/kedaluwarsa,
  import ulang file `.seb` terbaru

Test vector yang dipakai di `tests/`:

```
config-big-data.seb
  config key  ce349737793de563f17dd658c8fd29e6a856193342566cd289b01c2a0505cb45
  header utk  https://myklass-eng.umy.ac.id/mod/quiz/view.php?id=137097
              1af7f2a8bbb173cd0913f0e064e57ae1e4234c5c7d5eee83856254afb5dc2c44
```

```powershell
cargo test
```

## Cara header dipasang

`wry` tidak punya hook "tiap request", jadi WebTask memakai jalur WebView2
langsung:

- `WebViewExtWindows::webview()` → `ICoreWebView2`
- `AddWebResourceRequestedFilter` untuk `https://myklass-*/*` dan
  `https://*.umy.ac.id/*`, konteks `DOCUMENT`
- di handler `WebResourceRequested`, `req.Headers().SetHeader(...)` untuk
  `X-SafeExamBrowser-ConfigKeyHash` dan `X-SafeExamBrowser-RequestHash`

Handler COM harus jalan di UI thread yang sama dengan WebView. Itu sebabnya
hook dipasang setelah `build()` dan tidak ada request yang dimodifikasi dari
thread lain.

## Batasan (belum berubah dari versi Python)

- **Password quiz tidak disentuh.** Password disimpan hashed di server, diisi
  manual di form Moodle saat mulai attempt. Dosen selalu membagikannya.
- **Quiz bisa saja punya beberapa attempt** dan `reviewattempt` dimatikan;
  WebTask tidak memanen jawaban dan tidak mengubah attempt di server.
- **Config terenkripsi/binary ditolak** dengan pesan jelas. Minta versi `.seb`
  XML unencrypted.
- **Cloudflare**: `cf_clearance` terikat User-Agent dan IP. `scripts/login.py`
  menyimpan UA yang dipakai; jangan mengganti UA. Kalau clearance kedaluwarsa,
  ulangi skrip itu.
- **Belum diuji** pada quiz SEB yang benar-benar masih open. Yang sudah terbukti
  di repo ini: perhitungan config key (test vector), nilai header, injeksi header
  di request nyata (probe WebView2), dan penyimpanan config.
- **Windows-only**, karena memakai WebView2 dan `WebViewExtWindows`.

## Struktur

```
src/main.rs        entrypoint
src/keys.rs        parsing .seb + hitung config key + header per URL
src/store.rs       penyimpanan multi-config
src/server.rs      panel UI lokal (tiny_http)
src/app.rs         jendela tao + WebView wry: header hook, stub, verifikasi
src/ui/shell.html  panel drag & drop
scripts/login.py   login Cloudflare via Botasaurus (Python)
tests/keys.rs      test vector config key
DESIGN.md          alasan tertulis untuk keputusan warna, tipe, gerak
assets/logo.svg    logo aplikasi
assets/icon.ico    ikon jendela (dibuat dengan scripts/make_icon.py)
```

## Rilis

Push tag `v*` memicu `.github/workflows/release.yml`: build MSVC, jalankan test,
lalu unggah `webtask-<tag>-x86_64-pc-windows-msvc.zip` ke GitHub Release.

```bash
git tag v0.1.0
git push origin v0.1.0
```

Ikon jendela dibuat dari SVG:

```bash
pip install pillow
python scripts/make_icon.py
```

## Lisensi

MIT — lihat [LICENSE](LICENSE).

Proyek ini alat bantu pribadi untuk membuka quiz di lingkungan kampus sendiri.
Tanggung jawab mematuhi aturan akademik tetap ada pada pengguna.

## Nama

WebTask dipilih supaya tidak menyerempet merek pihak lain. Jangan memakai "SEB",
"Safe Exam Browser", atau merek kampus di nama, package, maupun UI turunan
proyek ini.