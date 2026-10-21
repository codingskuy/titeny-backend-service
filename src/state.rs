//! Keadaan bersama aplikasi.

use std::sync::Arc;

use sqlx::PgPool;

use crate::{auth::otp::PengirimOtp, config::Config, ratelimit::Pembatas};

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: Arc<Config>,
    pub pembatas: Arc<Pembatas>,
    pub pengirim_otp: Arc<dyn PengirimOtp>,
}

impl AppState {
    pub fn baru(db: PgPool, config: Config, pengirim_otp: Arc<dyn PengirimOtp>) -> Self {
        let pembatas = Arc::new(Pembatas::baru(config.batas_per_menit));
        Self {
            db,
            config: Arc::new(config),
            pembatas,
            pengirim_otp,
        }
    }
}
