//! `GET /api/v1/rekomendasi`: aksi prioritas per zona (paginasi keyset, terbaru dulu).

use axum::{extract::State, Json};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::{baca_cursor, buat_cursor, limit_sah};
use crate::{auth::extract::Pelaku, error::Hasil, json::Kueri, state::AppState};

#[derive(Deserialize)]
pub struct Param {
    zona: Option<String>,
    limit: Option<i64>,
    cursor: Option<String>,
}

#[derive(sqlx::FromRow, Serialize)]
struct Baris {
    id: String,
    zona: String,
    teks: Value,
    status: String,
    penanggung_jawab: Option<String>,
    target_tanggal: Option<NaiveDate>,
    catatan: Option<String>,
}

pub async fn rekomendasi(
    State(s): State<AppState>,
    pelaku: Pelaku,
    Kueri(p): Kueri<Param>,
) -> Hasil<Json<Value>> {
    pelaku.butuh_baca()?;
    let zona = pelaku.batasi_zona(p.zona.as_deref())?;
    let limit = limit_sah(p.limit)?;
    let cursor = match p.cursor.as_deref() {
        Some(c) => Some(baca_cursor(c, 1)?.remove(0)),
        None => None,
    };
    let mut baris: Vec<Baris> = sqlx::query_as(
        "SELECT id, zona, teks, status, penanggung_jawab, target_tanggal, catatan
         FROM rekomendasi
         WHERE ($1::text IS NULL OR zona = $1) AND ($2::text IS NULL OR id < $2)
         ORDER BY id DESC
         LIMIT $3",
    )
    .bind(&zona)
    .bind(&cursor)
    .bind(limit + 1)
    .fetch_all(&s.db)
    .await?;
    let next_cursor = if baris.len() as i64 > limit {
        baris.truncate(limit as usize);
        baris.last().map(|b| buat_cursor(&[&b.id]))
    } else {
        None
    };
    Ok(Json(json!({"data": baris, "next_cursor": next_cursor})))
}
