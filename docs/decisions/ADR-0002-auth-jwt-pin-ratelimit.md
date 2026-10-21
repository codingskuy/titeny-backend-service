# ADR-0002 — Auth JWT `aud=titeny`, PIN zona, dan pembatas laju

- **Status**: Diterima
- **Tanggal**: 2026-10-21

## Konteks

Tiket TTN-BE-004. FSD/30 dan ADR-0001 mewajibkan JWT `aud=titeny` `iss=chefgenie-auth` (akses 15 menit + refresh 7 hari dengan reuse-detection), PIN zona 6 digit untuk petani/vendor, dan batas 60 permintaan/menit per token. Beberapa dokumen produk menyebut "JWT 8 jam"; itu menyimpang dari standar repo.

## Keputusan

1. **TTL**: token akses 15 menit, refresh 7 hari. "8 jam" ditafsirkan sebagai batas umur SESI analis sejak OTP (FSD/30), bukan umur JWT: refresh analis ditolak bila `sesi_mulai` lebih dari 8 jam dan keluarga refresh dicabut (wajib OTP ulang).
2. **Algoritma**: HS256 dengan `JWT_SECRET` (min. 32 karakter) dari lingkungan; klaim `iss`, `aud`, `sub`, `peran`, `iat`, `exp`, `jti`. `aud` selain `titeny` atau `iss` selain `chefgenie-auth` dijawab 401 `tidak_berhak`.
3. **Refresh**: token opak 256 bit, hanya digest SHA-256 yang disimpan, berotasi sekali pakai, berkeluarga (`keluarga_id`). Pemakaian ulang token yang sudah dipakai mencabut seluruh keluarga (401).
4. **Login dua langkah** untuk `ceo`/`analis`: kata sandi argon2id lalu OTP 6 digit (berlaku 5 menit, maks 5 percobaan, sekali pakai). Peran `layanan` mendapat token langsung (tanpa OTP) dan hanya berlaku untuk `POST /api/v1/event/masuk`; endpoint baca menjawab 403 untuk peran ini.
5. **PIN zona**: header `X-Zona-PIN` (6 digit) dicocokkan ke digest `sha256(pepper||pin)` di tabel `zona_pin` (PIN unik per zona/peran). PIN hanya memberi akses baca zonanya sendiri (`Pelaku::batasi_zona`); tidak berlaku untuk endpoint staf/layanan.
6. **Pembatas laju**: jendela geser 60/menit di memori per instans; kunci = digest token Bearer, atau digest PIN, atau IP klien untuk permintaan tanpa kredensial. Lewat batas: 429 + `Retry-After`.
7. **Kanal OTP**: dokumen belum menetapkan kanal pengiriman. Antarmuka `PengirimOtp` disediakan; implementasi bawaan hanya mencatat ke log bila `OTP_LOG_DEV=true` (dev). Kanal produksi = keputusan terbuka.

## Konsekuensi

- Pembatas laju per instans: skala horizontal perlu penyimpanan bersama (Redis) di ADR lanjutan.
- PIN 6 digit (10^6 ruang) rentan tebak-paksa; mitigasi saat ini hanya pembatas laju per IP/PIN. Disarankan penguncian per zona/IP pada iterasi berikut.
- Akun, PIN zona, dan akun layanan dibuat lewat SQL/alat admin; belum ada endpoint pengelolaan.
- Akses JWT tidak dapat dicabut sebelum kedaluwarsa (maks 15 menit setelah keluarga refresh dicabut).
