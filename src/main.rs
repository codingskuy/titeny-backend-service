use std::{net::SocketAddr, sync::Arc};

use titeny::{app, auth::otp::PengirimLog, config::Config, db, state::AppState};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
    let config = Config::dari_env().unwrap_or_else(|e| {
        eprintln!("konfigurasi tidak sah: {e}");
        std::process::exit(1);
    });
    let pool = db::connect().await;
    sqlx::migrate!().run(&pool).await.expect("migrasi gagal");
    let pengirim = Arc::new(PengirimLog {
        aktif: config.otp_log_dev,
    });
    let bind = config.bind_addr.clone();
    let router = app::router(AppState::baru(pool, config, pengirim));
    let l = tokio::net::TcpListener::bind(&bind)
        .await
        .expect("gagal bind");
    tracing::info!(%bind, "titeny-backend-service berjalan");
    axum::serve(
        l,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .expect("server berhenti");
}
