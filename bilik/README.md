# Bilik

Ruang ujian tanpa kunci.

Bilik adalah browser kecil untuk quiz MyKlass UMY (`myklass-eng.umy.ac.id`, `myklass-mkwu.umy.ac.id`) yang dikunci plugin Moodle `quizaccess_seb` (Safe Exam Browser). Tarik file `.seb` yang dibagikan dosen, Bilik hitung config key-nya dan langsung membuka quiz. Setelah itu semuanya browser biasa: multi-tab, copy paste, screenshot, devtools.

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

Yang dikirim ke Moodle bukan key mentah, tapi hash per halaman (`access_manager::check_key`):

```
X-SafeExamBrowser-ConfigKeyHash: sha256(url_halaman + config_key)
```

Bilik menghitung itu di setiap request dokumen, plus memasang stub `window.SafeExamBrowser` supaya AMD module `quizaccess_seb/validate_quiz_access` di halaman attempt ikut lolos.

## Instalasi

```bash
pip install -r requirements.txt
python -m playwright install chromium
```

Butuh `botasaurus` + `pyvirtualdisplay` hanya untuk login (Cloudflare). Kalau sesi MyKlass sudah ada di profil Chromium, langkah login bisa dilewati.

## Pakai

```bash
# 1. login sekali (menyimpan cookie + UA ke ~/.bilik/cookies.json)
python3 scripts/login.py

# 2. jalankan Bilik
python3 -m bilik
```

Jendela pertama adalah panel Bilik. Tarik file `.seb` ke situ (atau klik "Pilih file .seb"), quiz langsung terbuka di tab baru.

Variabel environment:

| Env | Arti |
|---|---|
| `BILIK_PORT` | port panel UI (default 8765) |
| `BILIK_HEADLESS` | `1` untuk tanpa jendela |
| `BILIK_URL` | buka URL ini saat start |
| `DISPLAY` | X display kalau jalan di server (mis. `:99`) |

## Yang disimpan

```
~/.bilik/configs.json     daftar config + config aktif
~/.bilik/profile/         profil Chromium (sesi login ikut tersimpan)
~/.bilik/cookies.json     cookie hasil scripts/login.py
```

Setiap quiz punya file `.seb` sendiri, jadi Bilik menyimpan banyak config sekaligus dan memilih yang cocok **berdasarkan `cmid`**, bukan URL persis. Config menyimpan `startURL = view.php?id=NNN` sementara pengerjaan terjadi di `attempt.php?attempt=X&cmid=NNN`; matching by `cmid` itu wajib, kalau tidak header tidak akan terpasang di halaman attempt.

## Verifikasi

Bilik otomatis memanggil WS `quizaccess_seb_validate_quiz_keys` setiap halaman quiz dibuka, lalu menampilkan toast:

- hijau, `Bilik: config cocok` → `{"configkey": true, "browserexamkey": true}`
- merah, `Bilik: config tidak cocok...` → config untuk quiz ini salah/kedaluwarsa, import ulang file `.seb` terbaru

Test vector yang dipakai di `tests/`:

```
config-big-data.seb
  config key  ce349737793de563f17dd658c8fd29e6a856193342566cd289b01c2a0505cb45
  header utk  https://myklass-eng.umy.ac.id/mod/quiz/view.php?id=137097
              1af7f2a8bbb173cd0913f0e064e57ae1e4234c5c7d5eee83856254afb5dc2c44
```

```bash
python3 -m pytest tests/ -q
```

## Batasan

- **Password quiz tidak disentuh.** Password disimpan hashed di server, diisi manual di form Moodle saat mulai attempt. Dosen selalu membagikannya.
- **Quiz bisa saja punya beberapa attempt** dan `reviewattempt` dimatikan; Bilik tidak memanen jawaban dan tidak mengubah attempt di server.
- **Config terenkripsi/binary ditolak** dengan pesan jelas. Minta versi `.seb` XML unencrypted.
- **Cloudflare**: `cf_clearance` terikat User-Agent dan IP. Jangan mengganti UA di Bilik; kalau clearance kedaluwarsa, ulangi `scripts/login.py`.
- **Belum diuji** pada quiz SEB yang benar-benar masih open. Yang sudah terbukti: perhitungan config key, nilai header, injeksi header di request nyata, stub `window.SafeExamBrowser`, dan validasi WS mengembalikan `configkey: true` (lihat log `VERIFY OK`).

## Struktur

```
bilik/keys.py      parsing .seb + hitung config key + header per URL
bilik/store.py     penyimpanan multi-config
bilik/server.py    panel UI lokal (stdlib, tanpa dependency)
bilik/app.py       launcher Chromium: injeksi header, stub, verifikasi
bilik/ui/          panel drag & drop
scripts/login.py   login Cloudflare via Botasaurus
tests/             test vector config key
```

## Nama

Bilik dipilih supaya tidak menyerempet merek pihak lain. Jangan memakai "SEB", "Safe Exam Browser", atau merek kampus di nama, package, maupun UI turunan proyek ini.