//! `GET /api/v1/demand`: prediksi demand H+7 / H+30 per komoditas-zona.

use axum::{extract::State, response::Response};
use chrono::{DateTime, NaiveDate, Utc};
use serde::Deserialize;
use serde_json::{json, Value};

use super::{bulatkan2, cek_id, dengan_cache, wajib, CACHE_PREDIKSI};
use crate::{
    auth::extract::Pelaku,
    error::{AppError, Hasil},
    json::Kueri,
    state::AppState,
};

#[derive(Deserialize)]
pub struct Param {
    komoditas_id: Option<String>,
    zona: Option<String>,
    horizon: Option<String>,
}

#[derive(sqlx::FromRow)]
struct Baris {
    komoditas_id: String,
    zona: String,
    tanggal: NaiveDate,
    horizon: String,
    demand_prediksi_kg: f64,
    rentang_bawah_kg: Option<f64>,
    rentang_atas_kg: Option<f64>,
    sinyal: String,
    rekomendasi: Value,
    model_version: String,
    #[allow(dead_code)]
    dihitung_pada: DateTime<Utc>,
}

pub async fn demand(
    State(s): State<AppState>,
    pelaku: Pelaku,
    Kueri(p): Kueri<Param>,
) -> Hasil<Response> {
    pelaku.butuh_baca()?;
    let komoditas = wajib(&p.komoditas_id, "komoditas_id")?.to_string();
    cek_id(&komoditas, "cmd_", "komoditas_id")?;
    let zona = pelaku
        .batasi_zona(p.zona.as_deref())?
        .ok_or_else(|| AppError::validasi("zona wajib diisi.", "zona"))?;
    let horizon = p.horizon.unwrap_or_else(|| "H+7_total_kg".into());
    if !["H+7_total_kg", "H+30"].contains(&horizon.as_str()) {
        return Err(AppError::validasi(
            "horizon harus H+7_total_kg atau H+30.",
            "horizon",
        ));
    }

    // Prediksi terbaru (tanggal terbesar) untuk kombinasi ini.
    let baris: Option<Baris> = sqlx::query_as(
        "SELECT komoditas_id, zona, tanggal, horizon, demand_prediksi_kg, rentang_bawah_kg,
                rentang_atas_kg, sinyal, rekomendasi, model_version, dihitung_pada
         FROM prediksi_demand
         WHERE komoditas_id = $1 AND zona = $2 AND horizon = $3
         ORDER BY tanggal DESC LIMIT 1",
    )
    .bind(&komoditas)
    .bind(&zona)
    .bind(&horizon)
    .fetch_optional(&s.db)
    .await?;
    let Some(b) = baris else {
        let ada: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM prediksi_demand WHERE komoditas_id = $1)",
        )
        .bind(&komoditas)
        .fetch_one(&s.db)
        .await?;
        return Err(if ada {
            AppError::DataBelumCukup("Data demand belum cukup.".into())
        } else {
            AppError::tidak_ditemukan("Komoditas tidak dikenal.", Some("komoditas_id"))
        });
    };
    Ok(dengan_cache(
        CACHE_PREDIKSI,
        json!({
            "jenis": "demand", "komoditas_id": b.komoditas_id, "zona": b.zona, "tanggal": b.tanggal,
            "horizon": b.horizon, "demand_prediksi_kg": bulatkan2(b.demand_prediksi_kg),
            "rentang_bawah_kg": b.rentang_bawah_kg.map(bulatkan2),
            "rentang_atas_kg": b.rentang_atas_kg.map(bulatkan2),
            "sinyal": b.sinyal, "rekomendasi": b.rekomendasi, "model_version": b.model_version,
        }),
    ))
}
