//! Aksi tulis analis (desktop): validasi flag, tahan prediksi, antrean. Idempoten via `aksi_id`
//! dan tercatat di `audit_aksi` (retensi 2 tahun, append-only).

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};
use chrono::{Duration, NaiveDate};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::{Postgres, Transaction};

use crate::{
    auth::extract::Pelaku,
    error::{AppError, Hasil},
    event::validasi::{format_event_id, id_sah},
    insight::{baca_cursor, buat_cursor, hari_ini_wib, limit_sah},
    json::{Bodi, Kueri},
    state::AppState,
};

#[derive(Deserialize)]
pub struct ValidasiReq {
    aksi_id: String,
    event_id: String,
    keputusan: Keputusan,
    #[serde(default)]
    nilai_baru: Option<i64>,
    alasan: String,
}

#[derive(Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Keputusan {
    Setuju,
    KoreksiNilai,
    Tolak,
}

impl Keputusan {
    fn sebagai_str(self) -> &'static str {
        match self {
            Self::Setuju => "setuju",
            Self::KoreksiNilai => "koreksi_nilai",
            Self::Tolak => "tolak",
        }
    }
}

#[derive(Deserialize)]
pub struct TahanReq {
    aksi_id: String,
    komoditas_id: String,
    pasar_id: String,
    alasan: String,
    berlaku_sampai: NaiveDate,
}

pub fn aksi_id_sah(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

pub fn alasan_sah(a: &str) -> bool {
    let n = a.chars().count();
    (10..=140).contains(&n) && !a.trim().is_empty()
}

/// Header `X-Idempotency-Key` wajib dan sama dengan `aksi_id`.
fn cek_kunci(headers: &HeaderMap, aksi_id: &str) -> Hasil<()> {
    let k = headers
        .get("x-idempotency-key")
        .and_then(|v| v.to_str().ok())
        .filter(|v| !v.is_empty())
        .ok_or_else(|| {
            AppError::PermintaanBuruk(
                "Header X-Idempotency-Key wajib (sama dengan aksi_id).".into(),
            )
        })?;
    if k != aksi_id {
        return Err(AppError::validasi(
            "X-Idempotency-Key harus sama dengan aksi_id.",
            "X-Idempotency-Key",
        ));
    }
    Ok(())
}

/// Catat audit (siapa, kapan, apa, alasan). `Ok(false)` bila `aksi_id` sudah ada dengan jenis sama
/// (duplikat); jenis berbeda = konflik 409.
async fn catat_audit(
    tx: &mut Transaction<'_, Postgres>,
    aksi_id: &str,
    jenis: &str,
    pelaku: &str,
    apa: &Value,
    alasan: &str,
) -> Hasil<bool> {
    let r = sqlx::query(
        "INSERT INTO audit_aksi (aksi_id, jenis, pelaku, apa, alasan) VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (aksi_id) DO NOTHING",
    )
    .bind(aksi_id)
    .bind(jenis)
    .bind(pelaku)
    .bind(apa)
    .bind(alasan)
    .execute(&mut **tx)
    .await?;
    if r.rows_affected() == 1 {
        return Ok(true);
    }
    let ada: String = sqlx::query_scalar("SELECT jenis FROM audit_aksi WHERE aksi_id = $1")
        .bind(aksi_id)
        .fetch_one(&mut **tx)
        .await?;
    if ada == jenis {
        Ok(false)
    } else {
        Err(AppError::Konflik(
            "aksi_id sudah dipakai untuk jenis aksi lain.".into(),
        ))
    }
}

fn status(diterima: bool) -> Json<Value> {
    Json(json!({"status": if diterima { "diterima" } else { "duplikat_diabaikan" }}))
}

pub async fn validasi(
    State(s): State<AppState>,
    pelaku: Pelaku,
    headers: HeaderMap,
    Bodi(r): Bodi<ValidasiReq>,
) -> Hasil<Json<Value>> {
    let klaim = pelaku.butuh_analis()?;
    cek_kunci(&headers, &r.aksi_id)?;
    if !aksi_id_sah(&r.aksi_id) {
        return Err(AppError::validasi("aksi_id tidak sah.", "aksi_id"));
    }
    if !format_event_id(&r.event_id) {
        return Err(AppError::validasi("event_id tidak sah.", "event_id"));
    }
    if !alasan_sah(&r.alasan) {
        return Err(AppError::validasi(
            "Alasan harus 10-140 karakter.",
            "alasan",
        ));
    }
    match (r.keputusan, r.nilai_baru) {
        (Keputusan::KoreksiNilai, None) => {
            return Err(AppError::validasi(
                "nilai_baru wajib untuk koreksi_nilai.",
                "nilai_baru",
            ))
        }
        (Keputusan::KoreksiNilai, Some(n)) if n <= 0 => {
            return Err(AppError::validasi(
                "nilai_baru harus lebih dari 0.",
                "nilai_baru",
            ))
        }
        (Keputusan::Setuju | Keputusan::Tolak, Some(_)) => {
            return Err(AppError::validasi(
                "nilai_baru hanya untuk koreksi_nilai.",
                "nilai_baru",
            ))
        }
        _ => {}
    }

    let mut tx = s.db.begin().await?;
    let flag: Option<Option<String>> =
        sqlx::query_scalar("SELECT flag FROM event_masuk WHERE event_id = $1")
            .bind(&r.event_id)
            .fetch_optional(&mut *tx)
            .await?;
    let apa = json!({
        "jenis": "validasi", "event_id": r.event_id, "keputusan": r.keputusan.sebagai_str(),
        "nilai_baru": r.nilai_baru,
    });
    // Duplikat aksi_id dijawab dulu (idempoten), sebelum memeriksa keadaan event.
    if !catat_audit(&mut tx, &r.aksi_id, "validasi", &klaim.sub, &apa, &r.alasan).await? {
        tx.rollback().await?;
        return Ok(status(false));
    }
    match flag {
        None => {
            return Err(AppError::tidak_ditemukan(
                "Event tidak ditemukan.",
                Some("event_id"),
            ));
        }
        Some(Some(f)) if f == "perlu_verifikasi" => {}
        Some(_) => {
            return Err(AppError::Konflik(
                "Event tidak berstatus perlu_verifikasi.".into(),
            ));
        }
    }
    let masuk = sqlx::query(
        "INSERT INTO validasi_analis (aksi_id, event_id, keputusan, nilai_baru, alasan, analis_id)
         VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT (event_id) DO NOTHING",
    )
    .bind(&r.aksi_id)
    .bind(&r.event_id)
    .bind(r.keputusan.sebagai_str())
    .bind(r.nilai_baru)
    .bind(&r.alasan)
    .bind(&klaim.sub)
    .execute(&mut *tx)
    .await?;
    if masuk.rows_affected() == 0 {
        // tx dibatalkan saat dibuang: audit untuk aksi yang ditolak tidak ikut tersimpan.
        return Err(AppError::Konflik(
            "Event sudah diputuskan dengan aksi lain.".into(),
        ));
    }
    tx.commit().await?;
    Ok(status(true))
}

pub async fn tahan(
    State(s): State<AppState>,
    pelaku: Pelaku,
    headers: HeaderMap,
    Bodi(r): Bodi<TahanReq>,
) -> Hasil<Json<Value>> {
    let klaim = pelaku.butuh_analis()?;
    cek_kunci(&headers, &r.aksi_id)?;
    if !aksi_id_sah(&r.aksi_id) {
        return Err(AppError::validasi("aksi_id tidak sah.", "aksi_id"));
    }
    if !id_sah(&r.komoditas_id, "cmd_") {
        return Err(AppError::validasi(
            "komoditas_id tidak sah.",
            "komoditas_id",
        ));
    }
    if !id_sah(&r.pasar_id, "psr_") {
        return Err(AppError::validasi("pasar_id tidak sah.", "pasar_id"));
    }
    if !alasan_sah(&r.alasan) {
        return Err(AppError::validasi(
            "Alasan harus 10-140 karakter.",
            "alasan",
        ));
    }
    let hari_ini = hari_ini_wib();
    if r.berlaku_sampai < hari_ini || r.berlaku_sampai > hari_ini + Duration::days(7) {
        return Err(AppError::validasi(
            "berlaku_sampai harus dari hari ini sampai maksimal 7 hari ke depan.",
            "berlaku_sampai",
        ));
    }
    let apa = json!({
        "jenis": "tahan", "komoditas_id": r.komoditas_id, "pasar_id": r.pasar_id,
        "berlaku_sampai": r.berlaku_sampai,
    });
    let mut tx = s.db.begin().await?;
    if !catat_audit(&mut tx, &r.aksi_id, "tahan", &klaim.sub, &apa, &r.alasan).await? {
        tx.rollback().await?;
        return Ok(status(false));
    }
    sqlx::query(
        "INSERT INTO tahan_prediksi (aksi_id, komoditas_id, pasar_id, alasan, berlaku_sampai, analis_id)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(&r.aksi_id)
    .bind(&r.komoditas_id)
    .bind(&r.pasar_id)
    .bind(&r.alasan)
    .bind(r.berlaku_sampai)
    .bind(&klaim.sub)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(status(true))
}

#[derive(Deserialize)]
pub struct AntreanParam {
    limit: Option<i64>,
    cursor: Option<String>,
}

#[derive(sqlx::FromRow)]
struct BarisAntrean {
    event_id: String,
    event_type: String,
    alasan_flag: Option<String>,
}

/// `GET /api/v1/analis/antrean`: event `perlu_verifikasi` yang belum diputuskan (terlama dulu).
pub async fn antrean(
    State(s): State<AppState>,
    pelaku: Pelaku,
    Kueri(p): Kueri<AntreanParam>,
) -> Hasil<(StatusCode, Json<Value>)> {
    pelaku.butuh_analis()?;
    let limit = limit_sah(p.limit)?;
    let cursor = match p.cursor.as_deref() {
        Some(c) => Some(baca_cursor(c, 1)?.remove(0)),
        None => None,
    };
    let mut baris: Vec<BarisAntrean> = sqlx::query_as(
        "SELECT e.event_id, e.event_type,
                CASE e.event_type
                  WHEN 'harga.diperbarui' THEN 'jumlah_sampel_lapak kurang dari 3'
                  WHEN 'panen.dicatat' THEN 'hasil per hektare di luar 500-15.000 kg/ha'
                  ELSE NULL END AS alasan_flag
         FROM event_masuk e
         WHERE e.flag = 'perlu_verifikasi'
           AND NOT EXISTS (SELECT 1 FROM validasi_analis v WHERE v.event_id = e.event_id)
           AND ($1::text IS NULL OR e.event_id > $1)
         ORDER BY e.event_id
         LIMIT $2",
    )
    .bind(&cursor)
    .bind(limit + 1)
    .fetch_all(&s.db)
    .await?;
    let next_cursor = if baris.len() as i64 > limit {
        baris.truncate(limit as usize);
        baris.last().map(|b| buat_cursor(&[&b.event_id]))
    } else {
        None
    };
    let data: Vec<Value> = baris
        .into_iter()
        .map(|b| {
            json!({"event_id": b.event_id, "event_type": b.event_type, "flag": "perlu_verifikasi",
                   "alasan_flag": b.alasan_flag})
        })
        .collect();
    Ok((
        StatusCode::OK,
        Json(json!({"data": data, "next_cursor": next_cursor})),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alasan_10_sampai_140_karakter() {
        assert!(!alasan_sah("terlalu s"));
        assert!(alasan_sah("sepuluh ok"));
        assert!(alasan_sah(&"a".repeat(140)));
        assert!(!alasan_sah(&"a".repeat(141)));
        assert!(!alasan_sah("          "), "spasi saja tidak sah");
        assert!(
            alasan_sah(&"é".repeat(140)),
            "dihitung per karakter, bukan byte"
        );
    }

    #[test]
    fn format_aksi_id() {
        assert!(aksi_id_sah("aks_20261004_001"));
        assert!(!aksi_id_sah(""));
        assert!(!aksi_id_sah("a b"));
        assert!(!aksi_id_sah(&"a".repeat(65)));
    }
}
