//! `GET /api/v1/harga`: prediksi harga wajar H+1..H+7 untuk satu komoditas-pasar.

use axum::{extract::State, response::Response};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::{cek_id, dengan_cache, hari_ini_wib, ke_wib, wajib, CACHE_PREDIKSI};
use crate::{
    auth::extract::Pelaku,
    error::{AppError, Hasil},
    json::Kueri,
    state::AppState,
};

#[derive(Deserialize)]
pub struct Param {
    komoditas_id: Option<String>,
    pasar_id: Option<String>,
    dari: Option<NaiveDate>,
    sampai: Option<NaiveDate>,
}

#[derive(sqlx::FromRow, Serialize)]
struct Baris {
    #[sqlx(rename = "tanggal_prediksi")]
    tanggal: NaiveDate,
    horizon: String,
    harga_prediksi: i32,
    rentang_bawah: i32,
    rentang_atas: i32,
    kepercayaan: String,
    alasan_singkat: Option<String>,
    #[serde(skip)]
    model_version: String,
    #[serde(skip)]
    dihitung_pada: DateTime<Utc>,
}

pub async fn harga(
    State(s): State<AppState>,
    pelaku: Pelaku,
    Kueri(p): Kueri<Param>,
) -> Hasil<Response> {
    pelaku.butuh_baca()?;
    let komoditas = wajib(&p.komoditas_id, "komoditas_id")?.to_string();
    cek_id(&komoditas, "cmd_", "komoditas_id")?;
    // PIN zona: pasar default = zona sendiri; pasar lain ditolak 403.
    let pasar = pelaku
        .batasi_zona(p.pasar_id.as_deref())?
        .ok_or_else(|| AppError::validasi("pasar_id wajib diisi.", "pasar_id"))?;
    cek_id(&pasar, "psr_", "pasar_id")?;

    let hari_ini = hari_ini_wib();
    let batas_atas = hari_ini + Duration::days(7);
    let dari = p.dari.unwrap_or(hari_ini);
    let sampai = p.sampai.unwrap_or(batas_atas);
    if sampai > batas_atas {
        return Err(AppError::validasi(
            "sampai maksimum H+7 dari hari ini.",
            "sampai",
        ));
    }
    if dari > sampai {
        return Err(AppError::validasi(
            "dari tidak boleh setelah sampai.",
            "dari",
        ));
    }

    let baris: Vec<Baris> = sqlx::query_as(
        "SELECT tanggal_prediksi, horizon, harga_prediksi, rentang_bawah, rentang_atas, kepercayaan,
                alasan_singkat, model_version, dihitung_pada
         FROM prediksi_harga
         WHERE komoditas_id = $1 AND pasar_id = $2 AND tanggal_prediksi BETWEEN $3 AND $4
         ORDER BY tanggal_prediksi",
    )
    .bind(&komoditas)
    .bind(&pasar)
    .bind(dari)
    .bind(sampai)
    .fetch_all(&s.db)
    .await?;

    if baris.is_empty() {
        return Err(tidak_ada(&s, &komoditas, &pasar).await?);
    }
    let terbaru = baris
        .iter()
        .max_by_key(|b| b.dihitung_pada)
        .expect("tidak kosong");
    let (model_version, dihitung_pada) =
        (terbaru.model_version.clone(), ke_wib(terbaru.dihitung_pada));
    Ok(dengan_cache(
        CACHE_PREDIKSI,
        json!({
            "komoditas_id": komoditas, "pasar_id": pasar, "deret": baris,
            "model_version": model_version, "dihitung_pada": dihitung_pada,
        }),
    ))
}

/// Bedakan komoditas/pasar tak dikenal (`tidak_ditemukan`) dari data yang belum cukup.
async fn tidak_ada(s: &AppState, komoditas: &str, pasar: &str) -> Hasil<AppError> {
    let ada_k: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM prediksi_harga WHERE komoditas_id = $1)")
            .bind(komoditas)
            .fetch_one(&s.db)
            .await?;
    if !ada_k {
        return Ok(AppError::tidak_ditemukan(
            "Komoditas tidak dikenal.",
            Some("komoditas_id"),
        ));
    }
    let ada_p: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM prediksi_harga WHERE pasar_id = $1)")
            .bind(pasar)
            .fetch_one(&s.db)
            .await?;
    if !ada_p {
        return Ok(AppError::tidak_ditemukan(
            "Pasar tidak dikenal.",
            Some("pasar_id"),
        ));
    }
    Ok(AppError::DataBelumCukup("Data harga belum cukup.".into()))
}
