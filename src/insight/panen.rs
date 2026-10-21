//! `GET /api/v1/panen`: prediksi panen mingguan M+1..M+4 (paginasi keyset).

use axum::{extract::State, response::Response};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::{baca_cursor, buat_cursor, cek_id, dengan_cache, ke_wib, limit_sah, CACHE_PREDIKSI};
use crate::{
    auth::extract::Pelaku,
    error::{AppError, Hasil},
    json::Kueri,
    state::AppState,
};

#[derive(Deserialize)]
pub struct Param {
    komoditas_id: Option<String>,
    kabupaten_id: Option<String>,
    minggu: Option<String>,
    limit: Option<i64>,
    cursor: Option<String>,
}

#[derive(sqlx::FromRow, Serialize)]
struct Baris {
    #[sqlx(skip)]
    jenis: &'static str,
    komoditas_id: String,
    kabupaten_id: String,
    minggu: String,
    panen_prediksi_kg: i32,
    rentang_bawah_kg: Option<i32>,
    rentang_atas_kg: Option<i32>,
    kepercayaan: String,
    model_version: String,
    #[serde(skip)]
    dihitung_pada: DateTime<Utc>,
}

fn minggu_sah(m: &str) -> bool {
    let b = m.as_bytes();
    b.len() == 8
        && b[..4].iter().all(u8::is_ascii_digit)
        && &m[4..6] == "-W"
        && b[6..].iter().all(u8::is_ascii_digit)
}

pub async fn panen(
    State(s): State<AppState>,
    pelaku: Pelaku,
    Kueri(p): Kueri<Param>,
) -> Hasil<Response> {
    pelaku.butuh_baca()?;
    if let Some(k) = &p.komoditas_id {
        cek_id(k, "cmd_", "komoditas_id")?;
    }
    // PIN zona untuk panen = kabupaten sendiri.
    let kabupaten = pelaku.batasi_zona(p.kabupaten_id.as_deref())?;
    if let Some(k) = &kabupaten {
        cek_id(k, "kab_", "kabupaten_id")?;
    }
    if let Some(m) = &p.minggu {
        if !minggu_sah(m) {
            return Err(AppError::validasi(
                "minggu harus berformat YYYY-Www.",
                "minggu",
            ));
        }
    }
    let limit = limit_sah(p.limit)?;
    let (ck, cb, cm) = match p.cursor.as_deref() {
        Some(c) => {
            let v = baca_cursor(c, 3)?;
            (Some(v[0].clone()), Some(v[1].clone()), Some(v[2].clone()))
        }
        None => (None, None, None),
    };

    // Keyset pada (komoditas_id, kabupaten_id, minggu); ambil limit+1 untuk next_cursor.
    let mut baris: Vec<Baris> = sqlx::query_as(
        "SELECT komoditas_id, kabupaten_id, minggu, panen_prediksi_kg, rentang_bawah_kg, rentang_atas_kg,
                kepercayaan, model_version, dihitung_pada
         FROM prediksi_panen
         WHERE ($1::text IS NULL OR komoditas_id = $1)
           AND ($2::text IS NULL OR kabupaten_id = $2)
           AND ($3::text IS NULL OR minggu = $3)
           AND ($4::text IS NULL OR (komoditas_id, kabupaten_id, minggu) > ($4, $5, $6))
         ORDER BY komoditas_id, kabupaten_id, minggu
         LIMIT $7",
    )
    .bind(&p.komoditas_id)
    .bind(&kabupaten)
    .bind(&p.minggu)
    .bind(&ck)
    .bind(&cb)
    .bind(&cm)
    .bind(limit + 1)
    .fetch_all(&s.db)
    .await?;

    if baris.is_empty() && p.cursor.is_none() {
        return Err(AppError::DataBelumCukup("Data panen belum cukup.".into()));
    }
    let next_cursor = if baris.len() as i64 > limit {
        baris.truncate(limit as usize);
        baris
            .last()
            .map(|b| buat_cursor(&[&b.komoditas_id, &b.kabupaten_id, &b.minggu]))
    } else {
        None
    };
    let dihitung = baris.iter().map(|b| b.dihitung_pada).max().map(ke_wib);
    for b in &mut baris {
        b.jenis = "panen";
    }
    Ok(dengan_cache(
        CACHE_PREDIKSI,
        json!({"deret": baris, "dihitung_pada": dihitung, "next_cursor": next_cursor}),
    ))
}
