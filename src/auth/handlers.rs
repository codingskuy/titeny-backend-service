//! Endpoint `/api/v1/auth/*`.

use axum::{extract::State, Json};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use super::service::{self, HasilLogin};
use crate::{
    error::{AppError, Hasil},
    json::Bodi,
    state::AppState,
};

#[derive(Deserialize)]
pub struct LoginReq {
    username: String,
    password: String,
}

#[derive(Deserialize)]
pub struct OtpReq {
    sesi_id: String,
    otp: String,
}

#[derive(Deserialize)]
pub struct RefreshReq {
    refresh_token: String,
}

fn ke_json<T: serde::Serialize>(v: &T) -> Hasil<Json<Value>> {
    serde_json::to_value(v).map(Json).map_err(AppError::server)
}

pub async fn login(State(s): State<AppState>, Bodi(r): Bodi<LoginReq>) -> Hasil<Json<Value>> {
    if r.username.is_empty() {
        return Err(AppError::validasi("Username wajib diisi.", "username"));
    }
    match service::login(&s, &r.username, &r.password).await? {
        HasilLogin::Tantangan { sesi_id } => {
            Ok(Json(json!({"sesi_id": sesi_id, "otp_diperlukan": true})))
        }
        HasilLogin::Token(t) => ke_json(&t),
    }
}

pub async fn otp_verifikasi(
    State(s): State<AppState>,
    Bodi(r): Bodi<OtpReq>,
) -> Hasil<Json<Value>> {
    if r.otp.len() != 6 || !r.otp.bytes().all(|b| b.is_ascii_digit()) {
        return Err(AppError::validasi("OTP harus 6 digit.", "otp"));
    }
    let sesi_id = Uuid::parse_str(&r.sesi_id)
        .map_err(|_| AppError::validasi("sesi_id tidak sah.", "sesi_id"))?;
    ke_json(&service::verifikasi_otp(&s, sesi_id, &r.otp).await?)
}

pub async fn refresh(State(s): State<AppState>, Bodi(r): Bodi<RefreshReq>) -> Hasil<Json<Value>> {
    ke_json(&service::refresh(&s, &r.refresh_token).await?)
}
