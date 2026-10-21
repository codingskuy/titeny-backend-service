//! `GET /api/v1/kesehatan` (alias `/v1/kesehatan`): status, job terakhir, versi API.

use axum::{extract::State, response::Response};
use chrono::{DateTime, FixedOffset, Utc};
use serde_json::json;

use super::{dengan_cache, ke_wib, CACHE_KESEHATAN};
use crate::{error::Hasil, state::AppState};

pub const VERSI_API: &str = "v1.0.0";

pub async fn kesehatan(State(s): State<AppState>) -> Hasil<Response> {
    let terakhir: Option<DateTime<Utc>> = sqlx::query_scalar(
        "SELECT MAX(t) FROM (
           SELECT MAX(dihitung_pada) AS t FROM prediksi_harga
           UNION ALL SELECT MAX(dihitung_pada) FROM prediksi_panen
           UNION ALL SELECT MAX(dihitung_pada) FROM prediksi_demand
         ) x",
    )
    .fetch_one(&s.db)
    .await?;
    let job: Option<DateTime<FixedOffset>> = terakhir.map(ke_wib);
    Ok(dengan_cache(
        CACHE_KESEHATAN,
        json!({"status": "ok", "job_terakhir": job, "versi_api": VERSI_API}),
    ))
}
