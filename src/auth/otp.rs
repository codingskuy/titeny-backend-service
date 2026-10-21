//! Pengiriman OTP. Kanal pengiriman produksi belum ditetapkan dokumen;
//! `PengirimLog` hanya untuk pengembangan (`OTP_LOG_DEV=true`).

use async_trait::async_trait;

#[async_trait]
pub trait PengirimOtp: Send + Sync {
    async fn kirim(&self, username: &str, otp: &str);
}

/// Mencatat OTP ke log (HANYA dev). Bila `aktif=false`, OTP dibuang diam-diam.
pub struct PengirimLog {
    pub aktif: bool,
}

#[async_trait]
impl PengirimOtp for PengirimLog {
    async fn kirim(&self, username: &str, otp: &str) {
        if self.aktif {
            tracing::warn!(%username, %otp, "OTP (mode dev, jangan dipakai di produksi)");
        } else {
            tracing::warn!(%username, "OTP dibuat tetapi tidak ada kanal pengiriman terpasang");
        }
    }
}
