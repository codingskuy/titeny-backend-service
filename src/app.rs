//! Perakitan router `/api/v1`.

use std::time::Duration;

use axum::{
    extract::DefaultBodyLimit,
    http::StatusCode,
    middleware,
    routing::{get, post},
    Router,
};
use tower_http::{timeout::TimeoutLayer, trace::TraceLayer};

use crate::{auth, event, insight, ratelimit, state::AppState};

/// Bodi maks 256 KB (FSD/30).
pub const BATAS_BODI: usize = 256 * 1024;

pub fn router(state: AppState) -> Router {
    let v1 = Router::new()
        .route("/auth/login", post(auth::handlers::login))
        .route("/auth/otp/verifikasi", post(auth::handlers::otp_verifikasi))
        .route("/auth/refresh", post(auth::handlers::refresh))
        .route("/event/masuk", post(event::handler::masuk))
        .route("/harga", get(insight::harga::harga))
        .route("/demand", get(insight::demand::demand))
        .route("/panen", get(insight::panen::panen))
        .route("/rekomendasi", get(insight::rekomendasi::rekomendasi))
        .route("/kesehatan", get(insight::kesehatan::kesehatan));
    pelengkap(
        Router::new()
            .nest("/api/v1", v1)
            // Alias sesuai FSD/30: `GET /v1/kesehatan`.
            .route("/v1/kesehatan", get(insight::kesehatan::kesehatan))
            .route("/health", get(|| async { "ok" })),
        state,
    )
}

/// Lapisan umum: pembatas laju, batas bodi, timeout, trace. Dipakai juga oleh uji.
pub fn pelengkap(r: Router<AppState>, state: AppState) -> Router {
    r.layer(middleware::from_fn_with_state(
        state.clone(),
        ratelimit::batasi,
    ))
    .layer(DefaultBodyLimit::max(BATAS_BODI))
    .layer(TimeoutLayer::with_status_code(
        StatusCode::REQUEST_TIMEOUT,
        Duration::from_secs(10),
    ))
    .layer(TraceLayer::new_for_http())
    .with_state(state)
}
