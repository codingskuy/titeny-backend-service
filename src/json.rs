//! Ekstraktor bodi JSON dengan galat berbahasa Indonesia dan jalur field.
//!
//! JSON rusak -> 400; bentuk/tipe salah -> 422 `validasi_gagal` + nama field.

use axum::{
    body::Bytes,
    extract::{FromRequest, Request},
};
use serde::de::DeserializeOwned;

use crate::error::AppError;

pub struct Bodi<T>(pub T);

/// Uraikan bytes menjadi `T`. `Err((sintaks_rusak, pesan, field))`.
pub fn uraikan<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, (bool, String, Option<String>)> {
    let mut d = serde_json::Deserializer::from_slice(bytes);
    serde_path_to_error::deserialize::<_, T>(&mut d).map_err(|e| {
        let path = e.path().to_string();
        let field = if path.is_empty() || path == "." {
            None
        } else {
            Some(path)
        };
        let sintaks = e.inner().is_syntax() || e.inner().is_eof();
        (sintaks, e.inner().to_string(), field)
    })
}

impl<T, S> FromRequest<S> for Bodi<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let bytes = Bytes::from_request(req, state).await.map_err(|_| {
            AppError::PermintaanBuruk("Bodi tidak dapat dibaca atau melebihi 256 KB.".into())
        })?;
        match uraikan::<T>(&bytes) {
            Ok(v) => Ok(Bodi(v)),
            Err((true, _, _)) => Err(AppError::PermintaanBuruk(
                "Bodi bukan JSON yang sah.".into(),
            )),
            Err((false, pesan, field)) => Err(AppError::Validasi { pesan, field }),
        }
    }
}

/// Ekstraktor query string; tipe salah -> 400 berbahasa Indonesia.
pub struct Kueri<T>(pub T);

impl<T, S> axum::extract::FromRequestParts<S> for Kueri<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        serde_urlencoded::from_str::<T>(parts.uri.query().unwrap_or(""))
            .map(Kueri)
            .map_err(|_| {
                AppError::PermintaanBuruk("Parameter query tidak sah (salah tipe).".into())
            })
    }
}
