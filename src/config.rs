//! Konfigurasi dari variabel lingkungan.

use std::time::Duration;

pub const AUDIENS: &str = "titeny";
pub const PENERBIT: &str = "chefgenie-auth";
/// Umur token akses (standar repo: 15 menit).
pub const UMUR_ACCESS_DETIK: u64 = 15 * 60;
pub const UMUR_AKSES: Duration = Duration::from_secs(UMUR_ACCESS_DETIK);
/// Umur refresh token (standar repo: 7 hari).
pub const UMUR_REFRESH: Duration = Duration::from_secs(7 * 24 * 3600);
/// Batas umur sesi analis sejak OTP (FSD/30: 8 jam).
pub const UMUR_SESI_ANALIS: Duration = Duration::from_secs(8 * 3600);
/// Masa berlaku OTP.
pub const UMUR_OTP: Duration = Duration::from_secs(5 * 60);

#[derive(Clone, Debug)]
pub struct Config {
    pub jwt_secret: String,
    pub pin_pepper: String,
    pub batas_per_menit: u32,
    pub otp_log_dev: bool,
    pub bind_addr: String,
}

impl Config {
    /// Baca dari lingkungan. `JWT_SECRET` wajib (min. 32 karakter).
    pub fn dari_env() -> Result<Self, String> {
        let jwt_secret =
            std::env::var("JWT_SECRET").map_err(|_| "JWT_SECRET wajib diisi".to_string())?;
        if jwt_secret.len() < 32 {
            return Err("JWT_SECRET minimal 32 karakter".into());
        }
        let pin_pepper =
            std::env::var("PIN_PEPPER").unwrap_or_else(|_| format!("pin:{jwt_secret}"));
        let batas_per_menit = std::env::var("RATE_LIMIT_PER_MENIT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(60);
        let otp_log_dev = std::env::var("OTP_LOG_DEV")
            .map(|v| v == "true")
            .unwrap_or(false);
        let bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".into());
        Ok(Self {
            jwt_secret,
            pin_pepper,
            batas_per_menit,
            otp_log_dev,
            bind_addr,
        })
    }

    pub fn untuk_uji() -> Self {
        Self {
            jwt_secret: "rahasia-uji-rahasia-uji-rahasia-uji-123".into(),
            pin_pepper: "pepper-uji".into(),
            batas_per_menit: 60,
            otp_log_dev: false,
            bind_addr: "127.0.0.1:0".into(),
        }
    }
}
