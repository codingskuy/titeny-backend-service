-- TTN-BE-005: aksi analis idempoten + log audit (retensi 2 tahun, tidak dapat dihapus analis).
CREATE TABLE audit_aksi (
  id BIGSERIAL PRIMARY KEY,
  aksi_id TEXT NOT NULL UNIQUE,
  jenis TEXT NOT NULL CHECK (jenis IN ('validasi', 'tahan')),
  pelaku TEXT NOT NULL,
  waktu TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  apa JSONB NOT NULL,
  alasan TEXT NOT NULL,
  retensi_sampai TIMESTAMPTZ NOT NULL DEFAULT (NOW() + INTERVAL '2 years')
);

-- Append-only: tidak boleh diubah; hapus hanya setelah retensi 2 tahun lewat (pembersihan terjadwal).
CREATE FUNCTION audit_aksi_lindungi() RETURNS trigger AS $$
BEGIN
  IF TG_OP IN ('UPDATE', 'TRUNCATE') THEN
    RAISE EXCEPTION 'audit_aksi tidak dapat diubah atau dikosongkan';
  END IF;
  IF OLD.retensi_sampai > NOW() THEN
    RAISE EXCEPTION 'audit_aksi masih dalam masa retensi 2 tahun';
  END IF;
  RETURN OLD;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER audit_aksi_lindungi_trg
  BEFORE UPDATE OR DELETE ON audit_aksi
  FOR EACH ROW EXECUTE FUNCTION audit_aksi_lindungi();

CREATE TRIGGER audit_aksi_truncate_trg
  BEFORE TRUNCATE ON audit_aksi
  FOR EACH STATEMENT EXECUTE FUNCTION audit_aksi_lindungi();

CREATE TABLE validasi_analis (
  aksi_id TEXT PRIMARY KEY REFERENCES audit_aksi(aksi_id),
  event_id TEXT NOT NULL UNIQUE REFERENCES event_masuk(event_id),
  keputusan TEXT NOT NULL CHECK (keputusan IN ('setuju', 'koreksi_nilai', 'tolak')),
  nilai_baru BIGINT,
  alasan TEXT NOT NULL CHECK (char_length(alasan) BETWEEN 10 AND 140),
  analis_id TEXT NOT NULL,
  dibuat_pada TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CHECK ((keputusan = 'koreksi_nilai') = (nilai_baru IS NOT NULL))
);

CREATE TABLE tahan_prediksi (
  aksi_id TEXT PRIMARY KEY REFERENCES audit_aksi(aksi_id),
  komoditas_id TEXT NOT NULL,
  pasar_id TEXT NOT NULL,
  alasan TEXT NOT NULL CHECK (char_length(alasan) BETWEEN 10 AND 140),
  berlaku_sampai DATE NOT NULL,
  analis_id TEXT NOT NULL,
  dibuat_pada TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CHECK (berlaku_sampai <= (dibuat_pada AT TIME ZONE 'Asia/Jakarta')::date + 7)
);
CREATE INDEX tahan_prediksi_aktif_idx ON tahan_prediksi (komoditas_id, pasar_id, berlaku_sampai);
