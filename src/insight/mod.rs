//! Endpoint baca insight: harga, demand, panen, rekomendasi, kesehatan.
//! Hanya membaca DB `titeny`.

pub mod demand;
pub mod harga;
pub mod kesehatan;
pub mod panen;
pub mod rekomendasi;

use axum::{
    http::{header, HeaderValue},
    response::{IntoResponse, Response},
    Json,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{DateTime, FixedOffset, NaiveDate, Utc};
use serde::Serialize;

use crate::{
    error::{AppError, Hasil},
    event::validasi::id_sah,
};

/// Cache prediksi 6 jam.
pub const CACHE_PREDIKSI: &str = "public, max-age=21600";
/// Cache kesehatan 30 detik.
pub const CACHE_KESEHATAN: &str = "public, max-age=30";
pub const LIMIT_DEFAULT: i64 = 50;
pub const LIMIT_MAKS: i64 = 200;

pub fn wib() -> FixedOffset {
    FixedOffset::east_opt(7 * 3600).expect("offset WIB sah")
}

pub fn ke_wib(t: DateTime<Utc>) -> DateTime<FixedOffset> {
    t.with_timezone(&wib())
}

pub fn hari_ini_wib() -> NaiveDate {
    ke_wib(Utc::now()).date_naive()
}

/// Respons JSON dengan `Cache-Control` dan `Vary` (hasil bergantung pada kredensial/zona).
pub fn dengan_cache<T: Serialize>(cache: &'static str, body: T) -> Response {
    let mut r = Json(body).into_response();
    let h = r.headers_mut();
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
    h.insert(
        header::VARY,
        HeaderValue::from_static("Authorization, X-Zona-PIN"),
    );
    r
}

/// Validasi `limit` (default 50, maks 200).
pub fn limit_sah(limit: Option<i64>) -> Hasil<i64> {
    match limit {
        None => Ok(LIMIT_DEFAULT),
        Some(l) if (1..=LIMIT_MAKS).contains(&l) => Ok(l),
        Some(_) => Err(AppError::validasi("limit harus 1-200.", "limit")),
    }
}

/// Cursor opaque: base64url dari bagian kunci yang dipisah `|`.
pub fn buat_cursor(bagian: &[&str]) -> String {
    URL_SAFE_NO_PAD.encode(bagian.join("|"))
}

pub fn baca_cursor(cursor: &str, jumlah: usize) -> Hasil<Vec<String>> {
    let salah = || AppError::validasi("cursor tidak sah.", "cursor");
    let mentah = URL_SAFE_NO_PAD.decode(cursor).map_err(|_| salah())?;
    let teks = String::from_utf8(mentah).map_err(|_| salah())?;
    let bagian: Vec<String> = teks.split('|').map(str::to_string).collect();
    if bagian.len() == jumlah && bagian.iter().all(|b| !b.is_empty()) {
        Ok(bagian)
    } else {
        Err(salah())
    }
}

pub fn cek_id(nilai: &str, awalan: &str, field: &str) -> Hasil<()> {
    if id_sah(nilai, awalan) {
        Ok(())
    } else {
        Err(AppError::validasi(
            format!("{field} harus berformat {awalan}..."),
            field,
        ))
    }
}

pub fn wajib<'a>(v: &'a Option<String>, field: &str) -> Hasil<&'a str> {
    v.as_deref()
        .ok_or_else(|| AppError::validasi(format!("{field} wajib diisi."), field))
}

pub fn bulatkan2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_bolak_balik_dan_menolak_sampah() {
        let c = buat_cursor(&["cmd_padi", "kab_garut", "2026-W41"]);
        assert_eq!(
            baca_cursor(&c, 3).unwrap(),
            vec!["cmd_padi", "kab_garut", "2026-W41"]
        );
        assert!(baca_cursor(&c, 2).is_err());
        assert!(baca_cursor("!!!", 3).is_err());
    }

    #[test]
    fn batas_limit() {
        assert_eq!(limit_sah(None).unwrap(), 50);
        assert_eq!(limit_sah(Some(200)).unwrap(), 200);
        assert!(limit_sah(Some(0)).is_err());
        assert!(limit_sah(Some(201)).is_err());
    }
}
