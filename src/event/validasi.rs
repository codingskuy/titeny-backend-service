//! Validasi amplop dan payload 11 event terkunci (data/20-kontrak-event.md).
//!
//! Murni (tanpa DB): menerima `serde_json::Value`, mengembalikan event valid
//! beserta kunci idempoten per tipe, atau `Ditolak` (alasan + field).

use chrono::{DateTime, FixedOffset, NaiveDate, Utc};
use serde::Deserialize;
use serde_json::Value;

/// Registry TERKUNCI. Tambahan wajib lewat bump versi + dual-read 30 hari.
pub const EVENT_TERKUNCI: [&str; 11] = [
    "harga.diperbarui",
    "stok.berubah",
    "stok.snapshot_harian",
    "transaksi.dibuat",
    "lapak.status_berubah",
    "panen.dicatat",
    "logistik.biaya_diperbarui",
    "vendor.status_berubah",
    "cuaca.diperbarui",
    "kalender.dikunci",
    "keuangan.ringkasan_harian",
];

pub const SUMBER: [&str; 5] = ["lumbung", "titipo", "pasaree", "cuaca", "internal"];

#[derive(Debug, PartialEq, Eq)]
pub struct Ditolak {
    pub alasan: String,
    pub field: String,
}

impl Ditolak {
    fn gagal(field: impl Into<String>) -> Self {
        Self {
            alasan: "validasi_gagal".into(),
            field: field.into(),
        }
    }
}

#[derive(Debug)]
pub struct EventValid {
    pub event_id: String,
    pub event_type: String,
    pub schema_version: i32,
    pub source: String,
    pub occurred_at: DateTime<Utc>,
    pub received_at: DateTime<Utc>,
    pub batch_id: String,
    pub payload: Value,
    /// Kunci idempoten per tipe (diawali `event_type|`).
    pub kunci_idempoten: String,
    /// `perlu_verifikasi` bila payload ditandai/terdeteksi perlu ditinjau analis.
    pub flag: Option<String>,
}

// ---------- payload bertipe (enum/tanggal ditolak otomatis oleh serde) ----------

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Satuan {
    Kg,
}
#[derive(Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum FlagHarga {
    Ok,
    PerluVerifikasi,
}
#[derive(Deserialize)]
struct Harga {
    pasar_id: String,
    komoditas_id: String,
    tanggal: NaiveDate,
    harga_per_kg: i64,
    #[allow(dead_code)]
    satuan: Satuan,
    jumlah_sampel_lapak: i64,
    flag: FlagHarga,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum AlasanStok {
    MasukPanen,
    MasukBeli,
    KeluarPenjualan,
    Susut,
    KoreksiOpname,
}
#[derive(Deserialize)]
struct StokBerubah {
    gudang_id: String,
    komoditas_id: String,
    perubahan_kg: f64,
    stok_akhir_kg: f64,
    #[allow(dead_code)]
    alasan: AlasanStok,
}

#[derive(Deserialize)]
struct BarisStok {
    komoditas_id: String,
    stok_kg: f64,
}
#[derive(Deserialize)]
struct StokSnapshot {
    gudang_id: String,
    tanggal: NaiveDate,
    baris: Vec<BarisStok>,
    jumlah_baris: i64,
}

#[derive(Deserialize)]
struct Transaksi {
    transaksi_id: String,
    komoditas_id: String,
    qty_kg: f64,
    harga_satuan: i64,
    vendor_id: String,
    zona_rute: String,
    waktu: DateTime<FixedOffset>,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum StatusLapak {
    Aktif,
    Tutup,
    Libur,
}
#[derive(Deserialize)]
struct Lapak {
    lapak_id: String,
    #[allow(dead_code)]
    status: StatusLapak,
    komoditas_utama: String,
}

#[derive(Deserialize)]
enum Mutu {
    A,
    B,
    C,
}
#[derive(Deserialize)]
struct Panen {
    panen_id: String,
    komoditas_id: String,
    lahan_ha: f64,
    hasil_kg: f64,
    #[allow(dead_code)]
    tanggal_panen: NaiveDate,
    #[allow(dead_code)]
    mutu: Mutu,
    kabupaten_id: String,
    #[serde(default)]
    keterangan: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Moda {
    Motor,
    MobilPickup,
    Truk,
}
#[derive(Deserialize)]
struct Logistik {
    rute_id: String,
    jarak_km: f64,
    biaya_per_kg: i64,
    #[allow(dead_code)]
    moda: Moda,
    berlaku_mulai: NaiveDate,
    sebab: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum JenisVendor {
    Keliling,
    Lapak,
    Produsen,
}
#[derive(Deserialize)]
struct Vendor {
    vendor_id: String,
    #[allow(dead_code)]
    jenis: JenisVendor,
    #[allow(dead_code)]
    status_aktif: bool,
    zona_operasi: String,
    kapasitas_harian_kg: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Peringatan {
    TidakAda,
    Banjir,
    Kekeringan,
}
#[derive(Deserialize)]
struct Cuaca {
    kabupaten_id: String,
    tanggal: NaiveDate,
    curah_hujan_mm: f64,
    #[allow(dead_code)]
    suhu_avg_c: f64,
    #[allow(dead_code)]
    peringatan: Peringatan,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum JenisHari {
    Biasa,
    Puasa,
    Lebaran,
    Natal,
    TahunBaru,
    PanenRaya,
}
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Musim {
    Hujan,
    Kemarau,
    Peralihan,
}
#[derive(Deserialize)]
struct BarisKalender {
    #[allow(dead_code)]
    tanggal: NaiveDate,
    #[allow(dead_code)]
    jenis_hari: JenisHari,
    #[allow(dead_code)]
    musim: Musim,
}
#[derive(Deserialize)]
struct Kalender {
    tahun: i64,
    baris: Vec<BarisKalender>,
    jumlah_baris: i64,
    #[allow(dead_code)]
    dikunci_pada: DateTime<FixedOffset>,
}

#[derive(Deserialize)]
struct Keuangan {
    tanggal: NaiveDate,
    omzet_idr: i64,
    #[allow(dead_code)]
    margin_kotor_idr: i64,
    biaya_operasional_idr: i64,
    piutang_idr: i64,
    #[allow(dead_code)]
    r#final: bool,
}

// ---------- pembantu ----------

fn baca<T: for<'de> Deserialize<'de>>(payload: &Value) -> Result<T, Ditolak> {
    serde_path_to_error::deserialize::<_, T>(payload).map_err(|e| {
        let p = e.path().to_string();
        if p.is_empty() || p == "." {
            // Field hilang: serde melaporkan "missing field `nama`" pada objek induk.
            let pesan = e.inner().to_string();
            let hilang = pesan
                .strip_prefix("missing field `")
                .and_then(|r| r.split('`').next());
            return Ditolak::gagal(
                hilang.map_or("payload".to_string(), |h| format!("payload.{h}")),
            );
        }
        Ditolak::gagal(format!("payload.{p}"))
    })
}

pub fn id_sah(s: &str, awalan: &str) -> bool {
    s.len() > awalan.len()
        && s.len() <= 64
        && s.starts_with(awalan)
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

fn tak_kosong(s: &str) -> bool {
    !s.trim().is_empty() && s.len() <= 128
}

fn cek(syarat: bool, field: &str) -> Result<(), Ditolak> {
    if syarat {
        Ok(())
    } else {
        Err(Ditolak::gagal(field))
    }
}

fn nonneg(v: f64) -> bool {
    v.is_finite() && v >= 0.0
}

pub fn format_event_id(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 19
        && s.starts_with("evt_")
        && b[4..12].iter().all(u8::is_ascii_digit)
        && b[12] == b'_'
        && b[13..].iter().all(u8::is_ascii_digit)
}

fn waktu(v: &Value, field: &str) -> Result<DateTime<Utc>, Ditolak> {
    v.get(field)
        .and_then(Value::as_str)
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.with_timezone(&Utc))
        .ok_or_else(|| Ditolak::gagal(field))
}

fn teks<'a>(v: &'a Value, field: &str) -> Result<&'a str, Ditolak> {
    v.get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| Ditolak::gagal(field))
}

// ---------- titik masuk ----------

/// Validasi amplop + payload. `Ok` berisi event siap simpan.
pub fn validasi(masuk: &Value) -> Result<EventValid, Ditolak> {
    if !masuk.is_object() {
        return Err(Ditolak::gagal("event"));
    }
    let event_id = teks(masuk, "event_id")?.to_string();
    cek(format_event_id(&event_id), "event_id")?;

    let event_type = teks(masuk, "event_type")?.to_string();
    if !EVENT_TERKUNCI.contains(&event_type.as_str()) {
        return Err(Ditolak {
            alasan: "event_type_tidak_dikenal".into(),
            field: "event_type".into(),
        });
    }

    let schema_version = masuk
        .get("schema_version")
        .and_then(Value::as_i64)
        .filter(|v| (1..=i32::MAX as i64).contains(v))
        .ok_or_else(|| Ditolak::gagal("schema_version"))? as i32;

    let source = teks(masuk, "source")?.to_string();
    cek(SUMBER.contains(&source.as_str()), "source")?;
    let occurred_at = waktu(masuk, "occurred_at")?;
    let received_at = waktu(masuk, "received_at")?;
    let batch_id = teks(masuk, "batch_id")?.to_string();
    cek(tak_kosong(&batch_id), "batch_id")?;

    let payload = masuk
        .get("payload")
        .filter(|p| p.is_object())
        .ok_or_else(|| Ditolak::gagal("payload"))?;
    let (kunci, flag) = validasi_payload(&event_type, payload, &event_id, occurred_at)?;

    Ok(EventValid {
        event_id,
        kunci_idempoten: format!("{event_type}|{kunci}"),
        event_type,
        schema_version,
        source,
        occurred_at,
        received_at,
        batch_id,
        payload: payload.clone(),
        flag,
    })
}

const PV: &str = "perlu_verifikasi";

fn validasi_payload(
    tipe: &str,
    p: &Value,
    event_id: &str,
    occurred_at: DateTime<Utc>,
) -> Result<(String, Option<String>), Ditolak> {
    let ts = occurred_at.to_rfc3339();
    match tipe {
        "harga.diperbarui" => {
            let x: Harga = baca(p)?;
            cek(id_sah(&x.pasar_id, "psr_"), "payload.pasar_id")?;
            cek(id_sah(&x.komoditas_id, "cmd_"), "payload.komoditas_id")?;
            cek(x.harga_per_kg > 0, "payload.harga_per_kg")?;
            cek(x.jumlah_sampel_lapak >= 0, "payload.jumlah_sampel_lapak")?;
            // Sampel < 3 wajib bertanda perlu_verifikasi (kontrak 3.1).
            cek(
                x.jumlah_sampel_lapak >= 3 || x.flag == FlagHarga::PerluVerifikasi,
                "payload.jumlah_sampel_lapak",
            )?;
            let flag = (x.flag == FlagHarga::PerluVerifikasi).then(|| PV.to_string());
            Ok((
                format!("{}|{}|{}", x.pasar_id, x.komoditas_id, x.tanggal),
                flag,
            ))
        }
        "stok.berubah" => {
            let x: StokBerubah = baca(p)?;
            cek(tak_kosong(&x.gudang_id), "payload.gudang_id")?;
            cek(id_sah(&x.komoditas_id, "cmd_"), "payload.komoditas_id")?;
            cek(x.perubahan_kg.is_finite(), "payload.perubahan_kg")?;
            cek(nonneg(x.stok_akhir_kg), "payload.stok_akhir_kg")?;
            Ok((event_id.to_string(), None))
        }
        "stok.snapshot_harian" => {
            let x: StokSnapshot = baca(p)?;
            cek(tak_kosong(&x.gudang_id), "payload.gudang_id")?;
            cek(x.jumlah_baris >= 0, "payload.jumlah_baris")?;
            for (i, b) in x.baris.iter().enumerate() {
                cek(
                    id_sah(&b.komoditas_id, "cmd_"),
                    &format!("payload.baris[{i}].komoditas_id"),
                )?;
                cek(nonneg(b.stok_kg), &format!("payload.baris[{i}].stok_kg"))?;
            }
            Ok((format!("{}|{}", x.gudang_id, x.tanggal), None))
        }
        "transaksi.dibuat" => {
            let x: Transaksi = baca(p)?;
            cek(tak_kosong(&x.transaksi_id), "payload.transaksi_id")?;
            cek(id_sah(&x.komoditas_id, "cmd_"), "payload.komoditas_id")?;
            cek(
                x.qty_kg.is_finite() && x.qty_kg > 0.0 && x.qty_kg <= 50.0,
                "payload.qty_kg",
            )?;
            cek(x.harga_satuan >= 0, "payload.harga_satuan")?;
            cek(tak_kosong(&x.vendor_id), "payload.vendor_id")?;
            cek(tak_kosong(&x.zona_rute), "payload.zona_rute")?;
            let _ = x.waktu;
            Ok((x.transaksi_id, None))
        }
        "lapak.status_berubah" => {
            let x: Lapak = baca(p)?;
            cek(tak_kosong(&x.lapak_id), "payload.lapak_id")?;
            cek(
                id_sah(&x.komoditas_utama, "cmd_"),
                "payload.komoditas_utama",
            )?;
            Ok((format!("{}|{ts}", x.lapak_id), None))
        }
        "panen.dicatat" => {
            let x: Panen = baca(p)?;
            cek(tak_kosong(&x.panen_id), "payload.panen_id")?;
            cek(id_sah(&x.komoditas_id, "cmd_"), "payload.komoditas_id")?;
            cek(id_sah(&x.kabupaten_id, "kab_"), "payload.kabupaten_id")?;
            cek(
                x.lahan_ha.is_finite() && x.lahan_ha > 0.0,
                "payload.lahan_ha",
            )?;
            cek(
                x.hasil_kg.is_finite() && x.hasil_kg > 0.0,
                "payload.hasil_kg",
            )?;
            let per_ha = x.hasil_kg / x.lahan_ha;
            let di_luar = !(500.0..=15_000.0).contains(&per_ha);
            let ada_ket = x
                .keterangan
                .as_deref()
                .is_some_and(|k| !k.trim().is_empty());
            // Di luar 500-15.000 kg/ha: wajib `keterangan`, diterima sebagai outlier untuk analis.
            cek(!di_luar || ada_ket, "payload.hasil_kg")?;
            Ok((x.panen_id, di_luar.then(|| PV.to_string())))
        }
        "logistik.biaya_diperbarui" => {
            let x: Logistik = baca(p)?;
            cek(tak_kosong(&x.rute_id), "payload.rute_id")?;
            cek(
                x.jarak_km.is_finite() && x.jarak_km > 0.0,
                "payload.jarak_km",
            )?;
            cek(x.biaya_per_kg >= 0, "payload.biaya_per_kg")?;
            cek(tak_kosong(&x.sebab), "payload.sebab")?;
            Ok((format!("{}|{}", x.rute_id, x.berlaku_mulai), None))
        }
        "vendor.status_berubah" => {
            let x: Vendor = baca(p)?;
            cek(tak_kosong(&x.vendor_id), "payload.vendor_id")?;
            cek(tak_kosong(&x.zona_operasi), "payload.zona_operasi")?;
            cek(nonneg(x.kapasitas_harian_kg), "payload.kapasitas_harian_kg")?;
            Ok((format!("{}|{ts}", x.vendor_id), None))
        }
        "cuaca.diperbarui" => {
            let x: Cuaca = baca(p)?;
            cek(id_sah(&x.kabupaten_id, "kab_"), "payload.kabupaten_id")?;
            cek(nonneg(x.curah_hujan_mm), "payload.curah_hujan_mm")?;
            Ok((format!("{}|{}", x.kabupaten_id, x.tanggal), None))
        }
        "kalender.dikunci" => {
            let x: Kalender = baca(p)?;
            cek(x.tahun >= 2026, "payload.tahun")?;
            cek(x.jumlah_baris >= 0, "payload.jumlah_baris")?;
            let _ = x.baris.len();
            Ok((x.tahun.to_string(), None))
        }
        "keuangan.ringkasan_harian" => {
            let x: Keuangan = baca(p)?;
            cek(x.omzet_idr >= 0, "payload.omzet_idr")?;
            cek(
                x.biaya_operasional_idr >= 0,
                "payload.biaya_operasional_idr",
            )?;
            cek(x.piutang_idr >= 0, "payload.piutang_idr")?;
            Ok((x.tanggal.to_string(), None))
        }
        _ => Err(Ditolak {
            alasan: "event_type_tidak_dikenal".into(),
            field: "event_type".into(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn amplop(tipe: &str, payload: Value) -> Value {
        json!({
            "event_id": "evt_20261004_000123", "event_type": tipe, "schema_version": 1,
            "source": "pasaree", "occurred_at": "2026-10-04T07:00:00+07:00",
            "received_at": "2026-10-04T07:00:41+07:00", "batch_id": "b1", "payload": payload
        })
    }

    fn harga(extra: Value) -> Value {
        let mut p = json!({"pasar_id":"psr_jakarta_timur_01","komoditas_id":"cmd_cabai_merah","tanggal":"2026-10-04",
            "harga_per_kg":42000,"satuan":"kg","jumlah_sampel_lapak":6,"flag":"ok"});
        for (k, v) in extra.as_object().unwrap() {
            p[k] = v.clone();
        }
        p
    }

    #[test]
    fn kunci_harga_memakai_pasar_komoditas_tanggal() {
        let e = validasi(&amplop("harga.diperbarui", harga(json!({})))).unwrap();
        assert_eq!(
            e.kunci_idempoten,
            "harga.diperbarui|psr_jakarta_timur_01|cmd_cabai_merah|2026-10-04"
        );
        assert_eq!(e.flag, None);
    }

    #[test]
    fn tolak_harga_nol_dan_sampel_kurang_tanpa_flag() {
        let d = validasi(&amplop(
            "harga.diperbarui",
            harga(json!({"harga_per_kg":0})),
        ))
        .unwrap_err();
        assert_eq!(d, Ditolak::gagal("payload.harga_per_kg"));
        let d = validasi(&amplop(
            "harga.diperbarui",
            harga(json!({"jumlah_sampel_lapak":2})),
        ))
        .unwrap_err();
        assert_eq!(d, Ditolak::gagal("payload.jumlah_sampel_lapak"));
        let ok = validasi(&amplop(
            "harga.diperbarui",
            harga(json!({"jumlah_sampel_lapak":2,"flag":"perlu_verifikasi"})),
        ))
        .unwrap();
        assert_eq!(ok.flag.as_deref(), Some("perlu_verifikasi"));
    }

    #[test]
    fn tipe_tak_dikenal_dan_registry_11() {
        assert_eq!(EVENT_TERKUNCI.len(), 11);
        let d = validasi(&amplop("pedaree.stok.v1", json!({}))).unwrap_err();
        assert_eq!(d.alasan, "event_type_tidak_dikenal");
        assert_eq!(d.field, "event_type");
    }

    #[test]
    fn format_event_id() {
        assert!(super::format_event_id("evt_20261004_000123"));
        for buruk in [
            "evt_2026104_000123",
            "evt_20261004_00012",
            "EVT_20261004_000123",
            "evt_20261004-000123",
            "",
        ] {
            assert!(!super::format_event_id(buruk), "{buruk}");
        }
    }
}
