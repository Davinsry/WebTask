# DESIGN.md — WebTask

Alasan tertulis untuk keputusan visual. Setiap nilai di bawah punya sebab; nilai
yang tidak punya sebab tidak dipakai.

## Apa layar ini

WebTask bukan landing page. Ini jendela utilitas desktop yang muncul sebentar
sebelum ujian, dipakai oleh satu orang dalam keadaan tertekan waktu. Pekerjaan
layar ini: **memasukkan file `.seb`, lalu menunjukkan quiz benar-benar bisa
dibuka.** Satu keputusan utama: konfigurasi ini valid atau tidak.

Karena itu hierarki dibangun di sekitar *satu* aksi (drop/import) dan *satu*
fakta (status config), bukan baris statistik dan bukan dashboard.

## Palet

Identitas: alat akademik yang tenang dan bisa dipercaya. Bukan hiburan, bukan
developer tool, bukan produk AI. Maka: netral kertas hangat sebagai basis,
satu aksen biru-tinta untuk aksi, satu hijau untuk "valid", satu merah untuk
"tidak valid". Tidak ada gradien.

Mengapa bukan ungu: ungu-biru adalah palet default model dan langsung terbaca
sebagai "belum diputuskan". Tiga core + satu aksen sudah cukup untuk memisahkan
aksi, status, dan bahaya (R-29).

| Peran | Nilai | Alasan |
|---|---|---|
| `--bg` | `#FAF9F7` | kertas hangat, bukan putih steril; mata tidak silau saat latar layar gelap |
| `--surface` | `#FFFFFF` | kartu/panel harus terpisah dari latar tanpa shadow |
| `--ink` | `#16181D` | teks utama, kontras ±16:1 |
| `--ink-2` | `#5A5F6B` | teks sekunder, ≥4.5:1 di latar dan di permukaan |
| `--line` | `#E4E1DB` | batas struktural, hangat senada latar |
| `--line-2` | `#CFCAC1` | batas saat hover/fokus, lebih tegas |
| `--accent` | `#1F4E79` | aksi utama; biru-tinta, bukan biru terang default |
| `--ok` | `#1F7A4D` | status valid |
| `--bad` | `#B3261E` | status tidak valid |

Radius: `6px` untuk kontrol, `10px` untuk panel. Satu nilai besar (panel) cukup
untuk membedakan tingkat; tidak ada pill seragam (R-11).
Shadow: tidak ada, kecuali satu lapis tipis pada panel terangkat. Elevasi
dipakai sebagai penanda, bukan dekorasi (R-12).

## Tipografi

Sans sistem (`Segoe UI Variable`/`Segoe UI`) untuk UI. Alasan: ini aplikasi
Windows native, dan memuat webfont berarti ada teks yang berkedip saat file
dibuka. Tidak ada font "AI default" yang diimpor tanpa alasan (R-06).
Mono hanya untuk config key dan URL, karena isinya memang identifier.

Skala: 12 / 13 / 15 / 20 / 28. H1 28px — ini jendela alat, bukan hero
pemasaran. Tidak ada uppercase ber-tracking lebar.

## Gerak

Motion dial: **2/10**. Hanya transisi hover/fokus 120ms dan satu animasi status
saat verifikasi selesai. Tidak ada loop abadi, tidak ada reveal saat scroll
(halaman ini tidak menggulir secara naratif). `prefers-reduced-motion`
mematikan semuanya (R-19).

## Yang sengaja tidak ada

- Tidak ada badge di atas H1 yang mengulang H1 (R-09).
- Tidak ada emoji di teks UI (R-04).
- Tidak ada grid latar, blob, glow, atau glass (R-07, R-10, R-13).
- Tidak ada kartu fitur yang semuanya sama besar; daftar config adalah tabel
  keputusan, bukan grid kartu (R-14).
- Tidak ada angka statistik yang dikarang. Jumlah config yang tampil adalah
  jumlah nyata dari store (R-17, R-38).
- Tidak ada kontrol yang tidak berfungsi: setiap tombol di panel memanggil
  endpoint yang ada (R-26).
- Empty state menyebut sebab dan aksi berikutnya, bukan "No data" (R-27).

## Kosakata antarmuka

Bahasa Indonesia, kalimat kerja: "Tarik file .seb", "Import ulang file .seb",
"Config cocok". Status selalu kata + warna, tidak pernah warna saja (R-25).