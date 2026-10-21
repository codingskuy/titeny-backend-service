//! Ekstraktor identitas: JWT Bearer atau `X-Zona-PIN`.

use axum::{extract::FromRequestParts, http::request::Parts};

use super::{
    jwt::{self, Klaim},
    pin::{self, PelakuZona},
    Peran,
};
use crate::{
    error::{AppError, Hasil},
    state::AppState,
};

#[derive(Debug, Clone)]
pub enum Pelaku {
    Jwt(Klaim),
    Zona(PelakuZona),
}

impl Pelaku {
    /// Hanya token layanan (sumber event).
    pub fn butuh_layanan(&self) -> Hasil<&Klaim> {
        match self {
            Self::Jwt(k) if k.peran == Peran::Layanan => Ok(k),
            _ => Err(AppError::TidakBerhak(
                "Hanya token layanan yang diizinkan.".into(),
            )),
        }
    }

    /// CEO atau analis.
    pub fn butuh_staf(&self) -> Hasil<&Klaim> {
        match self {
            Self::Jwt(k) if matches!(k.peran, Peran::Ceo | Peran::Analis) => Ok(k),
            _ => Err(AppError::TidakBerhak("Peran tidak diizinkan.".into())),
        }
    }

    /// Hanya analis.
    pub fn butuh_analis(&self) -> Hasil<&Klaim> {
        match self {
            Self::Jwt(k) if k.peran == Peran::Analis => Ok(k),
            _ => Err(AppError::TidakBerhak("Hanya analis yang diizinkan.".into())),
        }
    }

    /// Akses baca papan: PIN zona atau JWT CEO/analis. Token layanan least-privilege: ditolak.
    pub fn butuh_baca(&self) -> Hasil<()> {
        match self {
            Self::Zona(_) => Ok(()),
            Self::Jwt(k) if matches!(k.peran, Peran::Ceo | Peran::Analis) => Ok(()),
            Self::Jwt(_) => Err(AppError::TidakBerhak(
                "Token layanan hanya untuk POST /event/masuk.".into(),
            )),
        }
    }

    /// Zona yang membatasi akses (PIN), `None` bila tak dibatasi (CEO/analis).
    pub fn zona_terbatas(&self) -> Option<&str> {
        match self {
            Self::Zona(z) => Some(&z.zona),
            Self::Jwt(_) => None,
        }
    }

    /// Pastikan nilai filter (jika ada) berada di zona PIN. Mengembalikan nilai efektif.
    pub fn batasi_zona(&self, diminta: Option<&str>) -> Hasil<Option<String>> {
        match (self.zona_terbatas(), diminta) {
            (Some(z), Some(d)) if z != d => {
                Err(AppError::TidakBerhak("Zona di luar PIN Anda.".into()))
            }
            (Some(z), _) => Ok(Some(z.to_string())),
            (None, d) => Ok(d.map(str::to_string)),
        }
    }

    /// Identitas untuk audit.
    pub fn id_audit(&self) -> String {
        match self {
            Self::Jwt(k) => k.sub.clone(),
            Self::Zona(z) => format!("zona:{}", z.zona),
        }
    }
}

impl FromRequestParts<AppState> for Pelaku {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        if let Some(h) = parts.headers.get(axum::http::header::AUTHORIZATION) {
            let h = h.to_str().map_err(|_| {
                AppError::TidakTerautentikasi("Header Authorization tidak sah.".into())
            })?;
            let token = h.strip_prefix("Bearer ").ok_or_else(|| {
                AppError::TidakTerautentikasi("Skema Authorization harus Bearer.".into())
            })?;
            return jwt::verifikasi(&state.config, token).map(Pelaku::Jwt);
        }
        if let Some(p) = parts.headers.get("x-zona-pin") {
            let p = p
                .to_str()
                .map_err(|_| AppError::TidakTerautentikasi("PIN zona tidak sah.".into()))?;
            return pin::cari(&state.db, &state.config, p)
                .await
                .map(Pelaku::Zona);
        }
        Err(AppError::TidakTerautentikasi(
            "Token atau PIN zona wajib.".into(),
        ))
    }
}
