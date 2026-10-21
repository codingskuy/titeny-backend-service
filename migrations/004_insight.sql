-- TTN-BE-003: tabel prediksi dan rekomendasi yang dibaca API (ditulis pipeline/model lewat titeny).
DROP TABLE IF EXISTS prediksi_harga;

CREATE TABLE prediksi_harga (
  komoditas_id TEXT NOT NULL,
  pasar_id TEXT NOT NULL,
  tanggal_prediksi DATE NOT NULL,
  horizon TEXT NOT NULL CHECK (horizon IN ('H+1','H+2','H+3','H+4','H+5','H+6','H+7')),
  harga_prediksi INTEGER NOT NULL CHECK (harga_prediksi >= 0 AND harga_prediksi % 500 = 0),
  rentang_bawah INTEGER NOT NULL,
  rentang_atas INTEGER NOT NULL CHECK (rentang_atas >= rentang_bawah),
  kepercayaan TEXT NOT NULL CHECK (kepercayaan IN ('tinggi','sedang','rendah')),
  alasan_singkat TEXT,
  model_version TEXT NOT NULL,
  dihitung_pada TIMESTAMPTZ NOT NULL,
  PRIMARY KEY (komoditas_id, pasar_id, tanggal_prediksi)
);

CREATE TABLE prediksi_panen (
  komoditas_id TEXT NOT NULL,
  kabupaten_id TEXT NOT NULL,
  minggu TEXT NOT NULL CHECK (minggu ~ '^\d{4}-W\d{2}$'),
  panen_prediksi_kg INTEGER NOT NULL CHECK (panen_prediksi_kg >= 0 AND panen_prediksi_kg % 50 = 0),
  rentang_bawah_kg INTEGER,
  rentang_atas_kg INTEGER,
  kepercayaan TEXT NOT NULL CHECK (kepercayaan IN ('tinggi','sedang','rendah')),
  model_version TEXT NOT NULL,
  dihitung_pada TIMESTAMPTZ NOT NULL,
  PRIMARY KEY (komoditas_id, kabupaten_id, minggu)
);

CREATE TABLE prediksi_demand (
  komoditas_id TEXT NOT NULL,
  zona TEXT NOT NULL,
  horizon TEXT NOT NULL CHECK (horizon IN ('H+7_total_kg','H+30')),
  tanggal DATE NOT NULL,
  demand_prediksi_kg DOUBLE PRECISION NOT NULL CHECK (demand_prediksi_kg >= 0),
  rentang_bawah_kg DOUBLE PRECISION,
  rentang_atas_kg DOUBLE PRECISION,
  sinyal TEXT NOT NULL CHECK (sinyal IN ('surplus','seimbang','defisit')),
  rekomendasi JSONB NOT NULL DEFAULT '[]'::jsonb,
  model_version TEXT NOT NULL,
  dihitung_pada TIMESTAMPTZ NOT NULL,
  PRIMARY KEY (komoditas_id, zona, horizon, tanggal)
);

CREATE TABLE rekomendasi (
  id TEXT PRIMARY KEY,
  zona TEXT NOT NULL,
  teks JSONB NOT NULL CHECK (jsonb_typeof(teks) = 'array' AND jsonb_array_length(teks) <= 3),
  status TEXT NOT NULL DEFAULT 'baru' CHECK (status IN ('baru','dikerjakan','selesai')),
  penanggung_jawab TEXT,
  target_tanggal DATE,
  catatan TEXT CHECK (catatan IS NULL OR char_length(catatan) <= 140),
  dibuat_pada TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX rekomendasi_zona_idx ON rekomendasi (zona, id DESC);
