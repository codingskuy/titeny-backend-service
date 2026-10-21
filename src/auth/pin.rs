//! PIN zona 6 digit untuk petani/vendor.

use sqlx::PgPool;

use super::password::digest;
use crate::{
    config::Config,
    error::{AppError, Hasil},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PelakuZona {
    pub zona: String,
    pub peran: String,
}

pub fn format_sah(pin: &str) -> bool {
    pin.len() == 6 && pin.bytes().all(|b| b.is_ascii_digit())
}

pub fn digest_pin(config: &Config, pin: &str) -> String {
    digest(&config.pin_pepper, pin)
}

pub async fn cari(db: &PgPool, config: &Config, pin: &str) -> Hasil<PelakuZona> {
    if !format_sah(pin) {
        return Err(AppError::TidakTerautentikasi(
            "PIN zona harus 6 digit.".into(),
        ));
    }
    let baris: Option<(String, String)> =
        sqlx::query_as("SELECT zona, peran FROM zona_pin WHERE pin_digest = $1 AND aktif")
            .bind(digest_pin(config, pin))
            .fetch_optional(db)
            .await?;
    baris
        .map(|(zona, peran)| PelakuZona { zona, peran })
        .ok_or_else(|| AppError::TidakTerautentikasi("PIN zona tidak dikenal.".into()))
}
