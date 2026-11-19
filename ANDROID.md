# Rencana: WebTask untuk Android

Status: rencana, belum ada kode.

## Ringkasan

WebTask versi Windows adalah Rust + WebView2 + `tao` + Win32 kiosk. Tidak ada
bagian yang bisa dipakai ulang di Android. Yang bisa dipindahkan adalah
**perilaku**, bukan kode:

1. Penyuntikan stub `SafeExamBrowser.security` (configKey/browserExamKey).
2. Header `X-SafeExamBrowser-ConfigKeyHash` / `-RequestHash` per request.
3. Penetralan anti-cheat MyKlass di sisi halaman.
4. Panel pemilihan config `.seb`.
5. Jendela kiosk (fullscreen, tanpa tombol sistem).

Target: satu aplikasi Android yang memuat MyKlass dengan header SEB yang benar,
anti-cheat dinetralkan, dan tampilan kiosk.

## Batasan yang harus disepakati dulu

### 1. Kunci perangkat (`startLockTask`)

| Mode | Syarat | Kekuatan |
| --- | --- | --- |
| Pinned screen | tidak ada | User bisa keluar lewat Back + Overview |
| Lock task penuh / kiosk | Device Owner via `adb shell dpm set-device-owner` | Keluar butuh Device Owner / restart with flag |

Perangkat Device Owner harus di-provision dari kondisi **belum ada akun Google**.
Kalau HP sudah dipakai sehari-hari, opsi realistis hanya pinned screen.

Keputusan yang dibutuhkan: apakah aplikasi ini untuk perangkat khusus ujian
(bisa Device Owner) atau HP pribadi (pinned screen saja)?

### 2. Header SEB di Android WebView

Windows: `ICoreWebView2WebResourceRequested` bisa menyisipkan header untuk
semua request dokumen.

Android:
- `WebViewClient.shouldInterceptRequest` hanya menangkap request yang lewat
  jaringan (subresource + navigasi tertentu). Navigasi `loadUrl` **tidak**
  melewati jalur ini secara konsisten di semua versi WebView.
- Header bisa ditambahkan lewat `loadUrl(url, headers)` — tapi hanya untuk
  request dokumen utama, dan hilang setelah redirect/navigasi internal Moodle.

Rencana mitigasi:
- Pakai `WebViewClient.shouldInterceptRequest` untuk subresource.
- Untuk navigasi, pakai `shouldOverrideUrlLoading` + `loadUrl(url, extraHeaders)`
  dengan nilai yang dihitung ulang per URL (hash = `sha256(url + configKey)`).
- Verifikasi terhadap test vector yang sudah terbukti bekerja di versi Windows:
  - config key: `ce349737793de563f17dd658c8fd29e6a856193342566cd289b01c2a0505cb45`
  - header untuk `https://myklass-eng.umy.ac.id/mod/quiz/view.php?id=137097`
    adalah `1af7f2a8bbb173cd0913f0e064e57ae1e4234c5c7d5eee83856254afb5dc2c44`

Kalau ternyata WebView Android membuang header pada redirect, perlu jalur
alternatif: `ServiceWorkerController` atau proxy lokal (lihat Langkah 6).

### 3. Parsing `.seb`

File `.seb` adalah plist XML berisi `SEBConfigFile` terenkripsi/base64. Logika
yang harus diport dari `src/keys.rs`:

- `parse_seb` / `plist_to_json`
- `config_key_from_data` (SHA-256 dari data config)
- `parse_cmid`, `host_of`, `header_value`

Di Android: `javax.crypto.Mac`/`MessageDigest` untuk SHA-256, dan parser plist
sederhana (tidak perlu library besar; format yang dipakai terbatas).

## Arsitektur

```
app/
  MainActivity.kt           host WebView + lifecycle
  kiosk/KioskController.kt  fullscreen, lock task, cegah tombol sistem
  seb/SebConfig.kt          model config (host, cmid, config_key)
  seb/SebStore.kt           baca/tulis daftar config (JSON di internal storage)
  seb/SebKeys.kt            sha256(url + key), header, parser .seb
  web/HeaderInterceptor.kt  shouldInterceptRequest + shouldOverrideUrlLoading
  web/InjectScripts.kt      stub SafeExamBrowser + penetral anti-cheat
  ui/PanelActivity.kt       daftar config, import .seb, aktivasi
  assets/panel.html         panel (port dari src/ui/shell.html)
```

Satu `WebView` per jendela, sama seperti versi desktop.

## Skrip yang dipindahkan apa adanya

`src/app.rs` → `InjectScripts.kt`.

Bagian ini murni JavaScript, tidak bergantung platform:

- `JS_SHA` (implementasi SHA-256 di JS) — sebenarnya tidak perlu lagi di
  Android karena hash dihitung native. Cukup sediakan `configKey` dan
  `browserExamKey` sebagai string jadi.
- `ANTICHEAT_NEUTRALIZE_JS` — pindah apa adanya. Isinya:
  - `document.body.id = 'x'` + buang class `path-mod-quiz`
  - hapus `#anti-cheat-overlay` + `MutationObserver`
  - `stopImmediatePropagation` pada `contextmenu`, `copy`, `cut`, `paste`,
    `mouseup`, `selectstart`, `keydown`, `keyup`
  - `blur` / `visibilitychange` dibatalkan, `document.hidden` dipaksa `false`
  - stylesheet `user-select:text!important`

Cara pasang di Android:

```kotlin
webView.webViewClient = object : WebViewClient() {
    override fun onPageStarted(view: WebView, url: String, favicon: Bitmap?) {
        view.evaluateJavascript(stubScript(url), null)
        view.evaluateJavascript(ANTICHEAT_NEUTRALIZE_JS, null)
    }
}
```

Catatan: `onPageStarted` berjalan setelah dokumen mulai dimuat, tidak selalu
sebelum script di BODY. Kalau urutan ini jadi masalah di halaman asli, opsi
kedua adalah `WebViewAssetLoader` yang menyajikan HTML yang sudah diinjeksi —
tapi itu merusak origin Moodle. Prioritaskan `onPageStarted` dulu, verifikasi
dengan halaman uji.

## Langkah kerja

1. **Kerangka proyek** — `settings.gradle.kts`, `app/build.gradle.kts`,
   `AndroidManifest.xml`, `MainActivity`. minSdk 26 (WebView modern), target 35.
2. **WebView dasar** — muat `https://myklass-eng.umy.ac.id`, aktifkan
   JavaScript, DOM storage, dan cookie.
3. **Stub SEB** — inject `window.SafeExamBrowser.security` dengan nilai dari
   config aktif; verifikasi lewat halaman uji yang mem-print `configKey`.
4. **Header** — `shouldInterceptRequest` + `shouldOverrideUrlLoading`;
   uji dengan test vector yang sama seperti versi Windows.
5. **Panel** — port `src/ui/shell.html` ke `assets/panel.html`, impor `.seb`
   lewat Storage Access Framework (`ACTION_OPEN_DOCUMENT`), simpan di
   `filesDir/configs.json`.
6. **Fallback header (kalau langkah 4 gagal)** — `ServiceWorkerController`
   atau proxy lokal `127.0.0.1` (mirip `tiny_http` versi desktop).
7. **Kiosk** — `WindowInsetsController` immersive, sembunyikan navigation bar,
   `startLockTask()` bila Device Owner; kalau tidak, minimal fullscreen.
8. **Penetralan anti-cheat** — pasang skrip, uji dengan halaman tiruan
   anti-cheat (sama seperti uji jsdom yang sudah dipakai untuk versi Windows).
9. **Uji lapangan** — buka quiz asli, cek: tidak ada dialog NTLM, tidak ada
   overlay hitam saat Alt+Tab/pindah app, klik kanan dan copy berfungsi,
   jawaban terkirim normal.

## Risiko

| Risiko | Dampak | Mitigasi |
| --- | --- | --- |
| Header hilang pada redirect | Quiz ditolak Moodle | Uji test vector; fallback proxy lokal |
| `onPageStarted` terlambat | Anti-cheat sempat aktif | Uji halaman asli; siapkan jalur asset loader |
| Device Owner tidak tersedia | Tidak bisa kunci penuh | Pinned screen + dokumentasikan batasnya |
| WebView Android versi lama | Fitur tidak konsisten | minSdk 26 + cek `WebViewCompat` |
| Update Chrome WebView | Perilaku berubah | Uji ulang tiap rilis WebView |

## Yang tidak dibawa

- `tao`, `wry`, WebView2, seluruh modul `kiosk` Win32.
- `windows_header_hook` (WebResourceRequested) → diganti interceptor.
- Server panel `tiny_http` → diganti panel native atau aset lokal.
- Fitur `with_additional_browser_args` (NTLM allowlist) → di Android tidak ada
  dialog Windows Security, jadi tidak diperlukan.

## Pertanyaan terbuka

1. Perangkat target bisa di-provision sebagai Device Owner, atau harus jalan di
   HP pribadi?
2. Perlu panel pemilihan config, atau cukup satu config aktif yang di-set lewat
   layar pengaturan?
3. Harus mendukung MKWU (v1, guard URL) sekaligus ENG (v2, guard body.id)?
   Skrip penetral saat ini menangani keduanya.
4. Distribusi: APK manual untuk diri sendiri, atau perlu siap dipasang orang
   lain (butuh signing + dokumentasi)?