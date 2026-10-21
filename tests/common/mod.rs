#![allow(dead_code)]
//! Pembantu uji integrasi.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use axum::{
    body::Body,
    http::{Request, StatusCode},
    routing::get,
    Router,
};
use http_body_util::BodyExt;
use serde_json::Value;
use sqlx::PgPool;
use titeny::{
    app,
    auth::{extract::Pelaku, otp::PengirimOtp, pin::digest_pin},
    config::Config,
    state::AppState,
};
use tower::ServiceExt;
use uuid::Uuid;

#[derive(Default)]
pub struct OtpTertangkap(pub Mutex<Vec<String>>);

#[async_trait]
impl PengirimOtp for OtpTertangkap {
    async fn kirim(&self, _username: &str, otp: &str) {
        self.0.lock().unwrap().push(otp.to_string());
    }
}

pub struct Uji {
    pub state: AppState,
    pub otp: Arc<OtpTertangkap>,
    pub router: Router,
}

pub fn siapkan(db: PgPool) -> Uji {
    siapkan_dengan(db, Config::untuk_uji())
}

pub fn siapkan_dengan(db: PgPool, config: Config) -> Uji {
    let otp = Arc::new(OtpTertangkap::default());
    let state = AppState::baru(db, config, otp.clone());
    let router = app::router(state.clone());
    Uji { state, otp, router }
}

/// Router uji dengan rute terlindungi sementara di atas lapisan umum aplikasi.
pub fn router_uji(state: &AppState) -> Router {
    let r = Router::new()
        .route(
            "/_uji/baca",
            get(|p: Pelaku| async move { p.butuh_baca().map(|_| "ok") }),
        )
        .route(
            "/_uji/layanan",
            get(|p: Pelaku| async move { p.butuh_layanan().map(|_| "ok") }),
        )
        .route(
            "/_uji/analis",
            get(|p: Pelaku| async move { p.butuh_analis().map(|_| "ok") }),
        )
        .route(
            "/_uji/zona",
            get(|p: Pelaku| async move { p.batasi_zona(None).map(|z| z.unwrap_or_default()) }),
        );
    app::pelengkap(r, state.clone())
}

pub async fn kirim(
    router: &Router,
    req: Request<Body>,
) -> (StatusCode, axum::http::HeaderMap, Value) {
    let resp = router.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let headers = resp.headers().clone();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let json = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into()));
    (status, headers, json)
}

pub fn post_json(path: &str, body: Value) -> Request<Body> {
    Request::post(path)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

pub fn get_dengan(path: &str, header: Option<(&str, &str)>) -> Request<Body> {
    let mut b = Request::get(path);
    if let Some((k, v)) = header {
        b = b.header(k, v);
    }
    b.body(Body::empty()).unwrap()
}

pub async fn sisip_pin(db: &PgPool, config: &Config, zona: &str, peran: &str, pin: &str) {
    sqlx::query("INSERT INTO zona_pin (id, zona, peran, pin_digest) VALUES ($1, $2, $3, $4)")
        .bind(Uuid::now_v7())
        .bind(zona)
        .bind(peran)
        .bind(digest_pin(config, pin))
        .execute(db)
        .await
        .unwrap();
}
