//! Handler `POST /api/v1/event/masuk`.

use axum::{
    body::Bytes,
    extract::{rejection::BytesRejection, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::{json, Value};

use super::validasi::{validasi, Ditolak};
use crate::{auth::extract::Pelaku, error::AppError, json::uraikan, state::AppState};

/// Galat khusus ingest: 422 memakai amplop `{status, alasan, field}` (kontrak event bagian 4).
pub enum GalatEvent {
    App(AppError),
    Ditolak(Ditolak),
}

impl From<AppError> for GalatEvent {
    fn from(e: AppError) -> Self {
        Self::App(e)
    }
}

impl From<sqlx::Error> for GalatEvent {
    fn from(e: sqlx::Error) -> Self {
        Self::App(e.into())
    }
}

impl IntoResponse for GalatEvent {
    fn into_response(self) -> Response {
        match self {
            Self::App(e) => e.into_response(),
            Self::Ditolak(d) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({"status": "ditolak", "alasan": d.alasan, "field": d.field})),
            )
                .into_response(),
        }
    }
}

pub async fn masuk(
    State(s): State<AppState>,
    pelaku: Pelaku,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> Result<Json<Value>, GalatEvent> {
    // Token layanan least-privilege: hanya sumber event yang boleh menulis di sini.
    pelaku.butuh_layanan()?;
    let kunci = headers
        .get("x-idempotency-key")
        .and_then(|v| v.to_str().ok())
        .filter(|v| !v.is_empty() && v.len() <= 64)
        .ok_or_else(|| {
            AppError::PermintaanBuruk(
                "Header X-Idempotency-Key wajib (sama dengan event_id).".into(),
            )
        })?;
    let body = body.map_err(|e| match e.status() {
        StatusCode::PAYLOAD_TOO_LARGE => AppError::BodiBesar,
        _ => AppError::PermintaanBuruk("Bodi tidak dapat dibaca.".into()),
    })?;
    let masuk: Value = uraikan(&body)
        .map_err(|_| AppError::PermintaanBuruk("Bodi bukan JSON yang sah.".into()))?;

    let ev = validasi(&masuk).map_err(GalatEvent::Ditolak)?;
    if kunci != ev.event_id {
        return Err(GalatEvent::Ditolak(Ditolak {
            alasan: "validasi_gagal".into(),
            field: "X-Idempotency-Key".into(),
        }));
    }

    // Duplikat event_id ATAU kunci idempoten per tipe => tidak ada baris ganda.
    let r = sqlx::query(
        "INSERT INTO event_masuk
           (event_id, event_type, schema_version, source, occurred_at, received_at, batch_id,
            payload, kunci_idempoten, flag)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
         ON CONFLICT DO NOTHING",
    )
    .bind(&ev.event_id)
    .bind(&ev.event_type)
    .bind(ev.schema_version)
    .bind(&ev.source)
    .bind(ev.occurred_at)
    .bind(ev.received_at)
    .bind(&ev.batch_id)
    .bind(&ev.payload)
    .bind(&ev.kunci_idempoten)
    .bind(&ev.flag)
    .execute(&s.db)
    .await?;

    let status = if r.rows_affected() == 0 {
        "duplikat_diabaikan"
    } else {
        "diterima"
    };
    Ok(Json(json!({ "status": status })))
}
