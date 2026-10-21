//! Penerbitan dan verifikasi JWT HS256 (`aud=titeny`, `iss=chefgenie-auth`).

use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::Peran;
use crate::{
    config::{Config, AUDIENS, PENERBIT, UMUR_AKSES},
    error::{AppError, Hasil},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Klaim {
    pub iss: String,
    pub aud: String,
    pub sub: String,
    pub peran: Peran,
    pub iat: i64,
    pub exp: i64,
    pub jti: String,
}

pub fn terbitkan(config: &Config, pengguna_id: Uuid, peran: Peran, sekarang: i64) -> Hasil<String> {
    let klaim = Klaim {
        iss: PENERBIT.into(),
        aud: AUDIENS.into(),
        sub: pengguna_id.to_string(),
        peran,
        iat: sekarang,
        exp: sekarang + UMUR_AKSES.as_secs() as i64,
        jti: Uuid::new_v4().to_string(),
    };
    encode(
        &Header::new(Algorithm::HS256),
        &klaim,
        &EncodingKey::from_secret(config.jwt_secret.as_bytes()),
    )
    .map_err(AppError::server)
}

/// Verifikasi tanda tangan, kedaluwarsa, `iss`, dan `aud`; audiens lain ditolak.
pub fn verifikasi(config: &Config, token: &str) -> Hasil<Klaim> {
    let mut v = Validation::new(Algorithm::HS256);
    v.set_audience(&[AUDIENS]);
    v.set_issuer(&[PENERBIT]);
    v.leeway = 0;
    v.set_required_spec_claims(&["exp", "aud", "iss", "sub"]);
    decode::<Klaim>(
        token,
        &DecodingKey::from_secret(config.jwt_secret.as_bytes()),
        &v,
    )
    .map(|d| d.claims)
    .map_err(|_| AppError::TidakTerautentikasi("Token tidak sah atau kedaluwarsa.".into()))
}
