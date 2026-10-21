-- TTN-BE-002: penyimpanan event masuk idempoten. Tabel lama (kerangka awal) diganti.
DROP TABLE IF EXISTS event_masuk;

CREATE TABLE event_masuk (
  event_id TEXT PRIMARY KEY,
  event_type TEXT NOT NULL,
  schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
  source TEXT NOT NULL CHECK (source IN ('lumbung', 'titipo', 'pasaree', 'cuaca', 'internal')),
  occurred_at TIMESTAMPTZ NOT NULL,
  received_at TIMESTAMPTZ NOT NULL,
  batch_id TEXT NOT NULL,
  payload JSONB NOT NULL,
  -- Kunci idempoten per tipe, diawali event_type (mis. harga: pasar_id|komoditas_id|tanggal).
  kunci_idempoten TEXT NOT NULL UNIQUE,
  flag TEXT CHECK (flag IS NULL OR flag = 'perlu_verifikasi'),
  diterima_pada TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX event_masuk_tipe_waktu_idx ON event_masuk (event_type, occurred_at);
CREATE INDEX event_masuk_flag_idx ON event_masuk (diterima_pada) WHERE flag = 'perlu_verifikasi';
