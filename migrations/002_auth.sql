-- TTN-BE-004: akun, OTP, refresh token berkeluarga, PIN zona.
CREATE TABLE pengguna (
  id UUID PRIMARY KEY,
  username TEXT NOT NULL UNIQUE,
  password_hash TEXT NOT NULL,
  peran TEXT NOT NULL CHECK (peran IN ('ceo', 'analis', 'layanan')),
  aktif BOOLEAN NOT NULL DEFAULT TRUE,
  dibuat_pada TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE sesi_otp (
  id UUID PRIMARY KEY,
  pengguna_id UUID NOT NULL REFERENCES pengguna(id),
  otp_hash TEXT NOT NULL,
  kedaluwarsa TIMESTAMPTZ NOT NULL,
  percobaan INTEGER NOT NULL DEFAULT 0,
  dipakai BOOLEAN NOT NULL DEFAULT FALSE,
  dibuat_pada TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE refresh_token (
  id UUID PRIMARY KEY,
  keluarga_id UUID NOT NULL,
  pengguna_id UUID NOT NULL REFERENCES pengguna(id),
  token_hash TEXT NOT NULL UNIQUE,
  sesi_mulai TIMESTAMPTZ NOT NULL,
  kedaluwarsa TIMESTAMPTZ NOT NULL,
  dipakai_pada TIMESTAMPTZ,
  dicabut BOOLEAN NOT NULL DEFAULT FALSE,
  dibuat_pada TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX refresh_token_keluarga_idx ON refresh_token (keluarga_id);

-- PIN zona 6 digit untuk petani/vendor; hanya digest (sha256 + pepper) yang disimpan.
CREATE TABLE zona_pin (
  id UUID PRIMARY KEY,
  zona TEXT NOT NULL,
  peran TEXT NOT NULL CHECK (peran IN ('petani', 'vendor')),
  pin_digest TEXT NOT NULL UNIQUE,
  aktif BOOLEAN NOT NULL DEFAULT TRUE,
  dibuat_pada TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
