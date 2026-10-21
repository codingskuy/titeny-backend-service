//! Logika login, OTP, dan refresh token berkeluarga dengan reuse-detection.

use chrono::{DateTime, Duration as Durasi, Utc};
use rand::Rng;
use serde::Serialize;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::{
    jwt,
    password::{cocok, digest, token_acak},
    Peran,
};
use crate::{
    config::{Config, UMUR_ACCESS_DETIK, UMUR_OTP, UMUR_REFRESH, UMUR_SESI_ANALIS},
    error::{AppError, Hasil},
    state::AppState,
};

#[derive(Debug, Serialize)]
pub struct TokenPasang {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: &'static str,
    pub expires_in: u64,
    pub peran: Peran,
}

#[derive(Debug)]
pub enum HasilLogin {
    Tantangan { sesi_id: Uuid },
    Token(TokenPasang),
}

fn tak_sah(pesan: &str) -> AppError {
    AppError::TidakTerautentikasi(pesan.into())
}

fn durasi(d: std::time::Duration) -> Durasi {
    Durasi::seconds(d.as_secs() as i64)
}

async fn terbitkan_pasang(
    conn: &mut PgConnection,
    config: &Config,
    pengguna_id: Uuid,
    peran: Peran,
    keluarga_id: Uuid,
    sesi_mulai: DateTime<Utc>,
) -> Hasil<TokenPasang> {
    let sekarang = Utc::now();
    let akses = jwt::terbitkan(config, pengguna_id, peran, sekarang.timestamp())?;
    let refresh = token_acak();
    sqlx::query(
        "INSERT INTO refresh_token (id, keluarga_id, pengguna_id, token_hash, sesi_mulai, kedaluwarsa)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(Uuid::now_v7())
    .bind(keluarga_id)
    .bind(pengguna_id)
    .bind(digest(&config.jwt_secret, &refresh))
    .bind(sesi_mulai)
    .bind(sekarang + durasi(UMUR_REFRESH))
    .execute(conn)
    .await?;
    Ok(TokenPasang {
        access_token: akses,
        refresh_token: refresh,
        token_type: "Bearer",
        expires_in: UMUR_ACCESS_DETIK,
        peran,
    })
}

pub async fn login(state: &AppState, username: &str, password: &str) -> Hasil<HasilLogin> {
    let baris: Option<(Uuid, String, String, bool)> =
        sqlx::query_as("SELECT id, password_hash, peran, aktif FROM pengguna WHERE username = $1")
            .bind(username)
            .fetch_optional(&state.db)
            .await?;
    let Some((id, hash, peran, aktif)) = baris else {
        return Err(tak_sah("Username atau kata sandi salah."));
    };
    if !cocok(password, &hash) || !aktif {
        return Err(tak_sah("Username atau kata sandi salah."));
    }
    let peran =
        Peran::dari_str(&peran).ok_or_else(|| AppError::server("peran tidak dikenal di DB"))?;
    match peran {
        Peran::Layanan => {
            let mut conn = state.db.acquire().await?;
            let pasang = terbitkan_pasang(
                &mut conn,
                &state.config,
                id,
                peran,
                Uuid::new_v4(),
                Utc::now(),
            )
            .await?;
            Ok(HasilLogin::Token(pasang))
        }
        Peran::Ceo | Peran::Analis => {
            let otp = format!("{:06}", rand::rngs::OsRng.gen_range(0..1_000_000u32));
            let sesi_id = Uuid::new_v4();
            sqlx::query("INSERT INTO sesi_otp (id, pengguna_id, otp_hash, kedaluwarsa) VALUES ($1, $2, $3, $4)")
                .bind(sesi_id)
                .bind(id)
                .bind(digest(&state.config.jwt_secret, &format!("{sesi_id}:{otp}")))
                .bind(Utc::now() + durasi(UMUR_OTP))
                .execute(&state.db)
                .await?;
            state.pengirim_otp.kirim(username, &otp).await;
            Ok(HasilLogin::Tantangan { sesi_id })
        }
    }
}

const MAKS_PERCOBAAN_OTP: i32 = 5;

#[allow(clippy::type_complexity)]
pub async fn verifikasi_otp(state: &AppState, sesi_id: Uuid, otp: &str) -> Hasil<TokenPasang> {
    let mut tx = state.db.begin().await?;
    let baris: Option<(Uuid, String, DateTime<Utc>, i32, bool, String, bool)> = sqlx::query_as(
        "SELECT s.pengguna_id, s.otp_hash, s.kedaluwarsa, s.percobaan, s.dipakai, p.peran, p.aktif
         FROM sesi_otp s JOIN pengguna p ON p.id = s.pengguna_id
         WHERE s.id = $1 FOR UPDATE OF s",
    )
    .bind(sesi_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((pengguna_id, otp_hash, kedaluwarsa, percobaan, dipakai, peran, aktif)) = baris else {
        return Err(tak_sah("OTP tidak sah."));
    };
    if dipakai || !aktif || kedaluwarsa < Utc::now() || percobaan >= MAKS_PERCOBAAN_OTP {
        return Err(tak_sah("OTP tidak sah."));
    }
    if digest(&state.config.jwt_secret, &format!("{sesi_id}:{otp}")) != otp_hash {
        sqlx::query("UPDATE sesi_otp SET percobaan = percobaan + 1 WHERE id = $1")
            .bind(sesi_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        return Err(tak_sah("OTP tidak sah."));
    }
    sqlx::query("UPDATE sesi_otp SET dipakai = TRUE WHERE id = $1")
        .bind(sesi_id)
        .execute(&mut *tx)
        .await?;
    let peran =
        Peran::dari_str(&peran).ok_or_else(|| AppError::server("peran tidak dikenal di DB"))?;
    let pasang = terbitkan_pasang(
        &mut tx,
        &state.config,
        pengguna_id,
        peran,
        Uuid::new_v4(),
        Utc::now(),
    )
    .await?;
    tx.commit().await?;
    Ok(pasang)
}

/// Rotasi refresh token. Pemakaian ulang token lama mencabut seluruh keluarga.
#[allow(clippy::type_complexity)]
pub async fn refresh(state: &AppState, refresh_token: &str) -> Hasil<TokenPasang> {
    let mut tx = state.db.begin().await?;
    let hash = digest(&state.config.jwt_secret, refresh_token);
    let baris: Option<(Uuid, Uuid, DateTime<Utc>, DateTime<Utc>, Option<DateTime<Utc>>, bool, String, bool)> =
        sqlx::query_as(
            "SELECT t.keluarga_id, t.pengguna_id, t.sesi_mulai, t.kedaluwarsa, t.dipakai_pada, t.dicabut,
                    p.peran, p.aktif
             FROM refresh_token t JOIN pengguna p ON p.id = t.pengguna_id
             WHERE t.token_hash = $1 FOR UPDATE OF t",
        )
        .bind(&hash)
        .fetch_optional(&mut *tx)
        .await?;
    let Some((keluarga, pengguna_id, sesi_mulai, kedaluwarsa, dipakai_pada, dicabut, peran, aktif)) =
        baris
    else {
        return Err(tak_sah("Refresh token tidak sah."));
    };
    let peran =
        Peran::dari_str(&peran).ok_or_else(|| AppError::server("peran tidak dikenal di DB"))?;
    let sekarang = Utc::now();

    let reuse = dipakai_pada.is_some();
    let sesi_habis = peran == Peran::Analis && sesi_mulai + durasi(UMUR_SESI_ANALIS) < sekarang;
    if reuse || sesi_habis {
        // Cabut seluruh keluarga dan pastikan pencabutan tersimpan sebelum menolak.
        sqlx::query("UPDATE refresh_token SET dicabut = TRUE WHERE keluarga_id = $1")
            .bind(keluarga)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        if reuse {
            tracing::warn!(%keluarga, "refresh token dipakai ulang; keluarga dicabut");
        }
        return Err(tak_sah("Sesi berakhir; masuk kembali."));
    }
    if dicabut || kedaluwarsa < sekarang || !aktif {
        return Err(tak_sah("Refresh token tidak sah."));
    }
    sqlx::query("UPDATE refresh_token SET dipakai_pada = $2 WHERE token_hash = $1")
        .bind(&hash)
        .bind(sekarang)
        .execute(&mut *tx)
        .await?;
    let pasang = terbitkan_pasang(
        &mut tx,
        &state.config,
        pengguna_id,
        peran,
        keluarga,
        sesi_mulai,
    )
    .await?;
    tx.commit().await?;
    Ok(pasang)
}

/// Dipakai uji dan alat admin: sisipkan akun.
pub async fn buat_pengguna(
    db: &PgPool,
    username: &str,
    password: &str,
    peran: Peran,
) -> Hasil<Uuid> {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO pengguna (id, username, password_hash, peran) VALUES ($1, $2, $3, $4)",
    )
    .bind(id)
    .bind(username)
    .bind(super::password::hash_password(password)?)
    .bind(peran.sebagai_str())
    .execute(db)
    .await?;
    Ok(id)
}
