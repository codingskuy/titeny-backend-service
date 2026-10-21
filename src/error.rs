//! `AppError` tunggal dengan amplop galat baku (FSD/30): 6 kode.

use axum::{
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("permintaan buruk: {0}")]
    PermintaanBuruk(String),
    #[error("bodi terlalu besar")]
    BodiBesar,
    #[error("validasi gagal: {pesan}")]
    Validasi {
        pesan: String,
        field: Option<String>,
    },
    #[error("tidak terautentikasi: {0}")]
    TidakTerautentikasi(String),
    #[error("tidak berhak: {0}")]
    TidakBerhak(String),
    #[error("tidak ditemukan: {pesan}")]
    TidakDitemukan {
        pesan: String,
        field: Option<String>,
    },
    #[error("data belum cukup: {0}")]
    DataBelumCukup(String),
    #[error("konflik: {0}")]
    Konflik(String),
    #[error("kuota habis")]
    KuotaHabis { retry_after: u64 },
    #[error("kesalahan server: {0}")]
    Server(String),
}

impl AppError {
    pub fn validasi(pesan: impl Into<String>, field: impl Into<String>) -> Self {
        Self::Validasi {
            pesan: pesan.into(),
            field: Some(field.into()),
        }
    }
    pub fn tidak_ditemukan(pesan: impl Into<String>, field: Option<&str>) -> Self {
        Self::TidakDitemukan {
            pesan: pesan.into(),
            field: field.map(Into::into),
        }
    }
    pub fn server(e: impl std::fmt::Display) -> Self {
        Self::Server(e.to_string())
    }
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        Self::Server(e.to_string())
    }
}

fn galat(status: StatusCode, kode: &str, pesan: &str, field: Option<&str>) -> Response {
    (
        status,
        Json(json!({"error": {"kode": kode, "pesan": pesan, "field": field}})),
    )
        .into_response()
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            Self::PermintaanBuruk(p) => galat(StatusCode::BAD_REQUEST, "validasi_gagal", &p, None),
            Self::BodiBesar => galat(
                StatusCode::PAYLOAD_TOO_LARGE,
                "validasi_gagal",
                "Bodi melebihi 256 KB.",
                None,
            ),
            Self::Validasi { pesan, field } => galat(
                StatusCode::UNPROCESSABLE_ENTITY,
                "validasi_gagal",
                &pesan,
                field.as_deref(),
            ),
            Self::TidakTerautentikasi(p) => {
                galat(StatusCode::UNAUTHORIZED, "tidak_berhak", &p, None)
            }
            Self::TidakBerhak(p) => galat(StatusCode::FORBIDDEN, "tidak_berhak", &p, None),
            Self::TidakDitemukan { pesan, field } => galat(
                StatusCode::NOT_FOUND,
                "tidak_ditemukan",
                &pesan,
                field.as_deref(),
            ),
            Self::DataBelumCukup(p) => galat(StatusCode::NOT_FOUND, "data_belum_cukup", &p, None),
            Self::Konflik(p) => galat(StatusCode::CONFLICT, "validasi_gagal", &p, None),
            Self::KuotaHabis { retry_after } => {
                let mut r = galat(
                    StatusCode::TOO_MANY_REQUESTS,
                    "kuota_habis",
                    "Kuota habis; coba lagi nanti.",
                    None,
                );
                if let Ok(v) = retry_after.to_string().parse() {
                    r.headers_mut().insert(header::RETRY_AFTER, v);
                }
                r
            }
            Self::Server(detail) => {
                tracing::error!(%detail, "kesalahan server");
                galat(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "kesalahan_server",
                    "Terjadi kesalahan di server.",
                    None,
                )
            }
        }
    }
}

pub type Hasil<T> = Result<T, AppError>;
