# CHANGELOG — titeny-backend-service

Format: satu seksi per rilis, terbaru di atas. Wajib diperbarui pada setiap commit `feat:` / `fix:` / `docs:`.

## [Belum dirilis]

### Ditambahkan
- Fondasi AI-dev v1.0.0: `AGENTS.md`, `CLAUDE.md`, `docs/decisions/ADR-0001-fondasi-teknis.md`, `versions/v1.0.0/docs/40-TECHSTACK.md` dan `60-BLAST-RADIUS.md` (versi lengkap, menggantikan ringkasan awal).
- Kontrak OpenAPI 3.1 v1.0.0 (`docs/api/openapi.yaml`) + referensi Scalar (`docs/api/index.html`) dan seksi "Dokumentasi API" di README.
- TTN-BE-004: auth JWT HS256 `aud=titeny` `iss=chefgenie-auth` (akses 15 menit, refresh 7 hari berkeluarga dengan reuse-detection), login analis/CEO dengan OTP (sesi analis maks 8 jam), token layanan least-privilege, PIN zona 6 digit (`X-Zona-PIN`), pembatas laju 60/menit (429 + `Retry-After`), `AppError` dengan 6 kode galat baku, router `/api/v1` dan ADR-0002.
- TTN-BE-002: `POST /api/v1/event/masuk` menerima 11 event terkunci dengan validasi amplop+payload, idempotensi per tipe (`event_id` atau kunci per tipe), 200 `duplikat_diabaikan`, 422 `{status, alasan, field}`; migrasi `003_event.sql` (tabel `event_masuk`).

### Diubah
- Migrasi Axum 0.7.9 ke 0.8.4; kode lama `/ingest/event` dan `/insight/*` dihapus (diganti `/api/v1/*` pada tiket berikutnya).
- `POST /api/v1/auth/login` dapat langsung menerbitkan token untuk peran `layanan` (kontrak OpenAPI diperbarui; `oneOf` LoginTantangan/TokenPasang).
