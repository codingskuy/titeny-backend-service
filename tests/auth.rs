mod common;

use axum::http::StatusCode;
use common::*;
use jsonwebtoken::{encode, EncodingKey, Header};
use serde_json::{json, Value};
use sqlx::PgPool;
use titeny::{
    auth::{jwt, service::buat_pengguna, Peran},
    config::Config,
};
use uuid::Uuid;

fn bearer(t: &str) -> String {
    format!("Bearer {t}")
}

async fn login_layanan(u: &Uji) -> Value {
    let (st, _, b) = kirim(
        &u.router,
        post_json(
            "/api/v1/auth/login",
            json!({"username":"lumbung","password":"rahasia123"}),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{b}");
    b
}

#[sqlx::test]
async fn login_layanan_mengeluarkan_jwt_15_menit_aud_titeny(db: PgPool) {
    let u = siapkan(db.clone());
    buat_pengguna(&db, "lumbung", "rahasia123", Peran::Layanan)
        .await
        .unwrap();
    let b = login_layanan(&u).await;
    assert_eq!(b["token_type"], "Bearer");
    assert_eq!(b["expires_in"], 900);
    assert_eq!(b["peran"], "layanan");
    let k = jwt::verifikasi(&u.state.config, b["access_token"].as_str().unwrap()).unwrap();
    assert_eq!(k.aud, "titeny");
    assert_eq!(k.iss, "chefgenie-auth");
    assert_eq!(k.exp - k.iat, 900);
}

#[sqlx::test]
async fn login_salah_kata_sandi_401_seragam(db: PgPool) {
    let u = siapkan(db.clone());
    buat_pengguna(&db, "lumbung", "rahasia123", Peran::Layanan)
        .await
        .unwrap();
    for (user, pw) in [("lumbung", "salah"), ("tidakada", "rahasia123")] {
        let (st, _, b) = kirim(
            &u.router,
            post_json("/api/v1/auth/login", json!({"username":user,"password":pw})),
        )
        .await;
        assert_eq!(st, StatusCode::UNAUTHORIZED);
        assert_eq!(b["error"]["kode"], "tidak_berhak");
    }
}

#[sqlx::test]
async fn bodi_rusak_400_dan_field_hilang_422(db: PgPool) {
    let u = siapkan(db);
    let req = axum::http::Request::post("/api/v1/auth/login")
        .body(axum::body::Body::from("{bukan json"))
        .unwrap();
    let (st, _, _) = kirim(&u.router, req).await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    let (st, _, b) = kirim(
        &u.router,
        post_json("/api/v1/auth/login", json!({"username":"x"})),
    )
    .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(b["error"]["kode"], "validasi_gagal");
}

#[sqlx::test]
async fn analis_wajib_otp_sebelum_token(db: PgPool) {
    let u = siapkan(db.clone());
    buat_pengguna(&db, "dewi", "rahasia123", Peran::Analis)
        .await
        .unwrap();
    let (st, _, b) = kirim(
        &u.router,
        post_json(
            "/api/v1/auth/login",
            json!({"username":"dewi","password":"rahasia123"}),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(b["otp_diperlukan"], true);
    assert!(b.get("access_token").is_none());
    let sesi = b["sesi_id"].as_str().unwrap().to_string();
    let otp = u.otp.0.lock().unwrap().last().cloned().unwrap();

    // OTP salah ditolak, lalu OTP benar berhasil sekali saja.
    let (st, _, _) = kirim(
        &u.router,
        post_json(
            "/api/v1/auth/otp/verifikasi",
            json!({"sesi_id":sesi,"otp":"000000"}),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
    let (st, _, b) = kirim(
        &u.router,
        post_json(
            "/api/v1/auth/otp/verifikasi",
            json!({"sesi_id":sesi,"otp":otp}),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{b}");
    assert_eq!(b["peran"], "analis");
    let (st, _, _) = kirim(
        &u.router,
        post_json(
            "/api/v1/auth/otp/verifikasi",
            json!({"sesi_id":sesi,"otp":otp}),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::UNAUTHORIZED, "OTP sekali pakai");
}

#[sqlx::test]
async fn refresh_berotasi_dan_reuse_mencabut_seluruh_keluarga(db: PgPool) {
    let u = siapkan(db.clone());
    buat_pengguna(&db, "lumbung", "rahasia123", Peran::Layanan)
        .await
        .unwrap();
    let t1 = login_layanan(&u).await;
    let r1 = t1["refresh_token"].as_str().unwrap().to_string();

    let (st, _, t2) = kirim(
        &u.router,
        post_json("/api/v1/auth/refresh", json!({"refresh_token": r1})),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let r2 = t2["refresh_token"].as_str().unwrap().to_string();
    assert_ne!(r1, r2);

    // Refresh lama dipakai lagi => 401 dan seluruh keluarga (termasuk r2) dicabut.
    let (st, _, _) = kirim(
        &u.router,
        post_json("/api/v1/auth/refresh", json!({"refresh_token": r1})),
    )
    .await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
    let (st, _, _) = kirim(
        &u.router,
        post_json("/api/v1/auth/refresh", json!({"refresh_token": r2})),
    )
    .await;
    assert_eq!(st, StatusCode::UNAUTHORIZED, "keluarga harus sudah dicabut");
    let dicabut: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM refresh_token WHERE dicabut")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(dicabut, 2);
}

#[sqlx::test]
async fn refresh_tak_dikenal_401(db: PgPool) {
    let u = siapkan(db);
    let (st, _, _) = kirim(
        &u.router,
        post_json("/api/v1/auth/refresh", json!({"refresh_token":"ngawur"})),
    )
    .await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn sesi_analis_lewat_8_jam_wajib_login_ulang(db: PgPool) {
    let u = siapkan(db.clone());
    buat_pengguna(&db, "dewi", "rahasia123", Peran::Analis)
        .await
        .unwrap();
    let (_, _, b) = kirim(
        &u.router,
        post_json(
            "/api/v1/auth/login",
            json!({"username":"dewi","password":"rahasia123"}),
        ),
    )
    .await;
    let otp = u.otp.0.lock().unwrap().last().cloned().unwrap();
    let (_, _, t) = kirim(
        &u.router,
        post_json(
            "/api/v1/auth/otp/verifikasi",
            json!({"sesi_id":b["sesi_id"],"otp":otp}),
        ),
    )
    .await;
    sqlx::query("UPDATE refresh_token SET sesi_mulai = NOW() - INTERVAL '9 hours'")
        .execute(&db)
        .await
        .unwrap();
    let (st, _, _) = kirim(
        &u.router,
        post_json(
            "/api/v1/auth/refresh",
            json!({"refresh_token": t["refresh_token"]}),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
}

fn jwt_buatan(config: &Config, aud: &str, iss: &str, exp_offset: i64, peran: &str) -> String {
    let now = chrono::Utc::now().timestamp();
    let klaim = json!({"iss":iss,"aud":aud,"sub":Uuid::new_v4().to_string(),"peran":peran,"iat":now,"exp":now+exp_offset,"jti":"x"});
    encode(
        &Header::default(),
        &klaim,
        &EncodingKey::from_secret(config.jwt_secret.as_bytes()),
    )
    .unwrap()
}

#[sqlx::test]
async fn jwt_audiens_selain_titeny_ditolak_401(db: PgPool) {
    let u = siapkan(db);
    let r = router_uji(&u.state);
    for aud in ["lumbung", "pasaree", "chefgenie"] {
        let t = jwt_buatan(&u.state.config, aud, "chefgenie-auth", 600, "analis");
        let (st, _, b) = kirim(
            &r,
            get_dengan("/_uji/baca", Some(("authorization", &bearer(&t)))),
        )
        .await;
        assert_eq!(st, StatusCode::UNAUTHORIZED, "aud={aud}");
        assert_eq!(b["error"]["kode"], "tidak_berhak");
    }
    // kontrol: aud benar diterima; iss salah dan kedaluwarsa ditolak.
    let ok = jwt_buatan(&u.state.config, "titeny", "chefgenie-auth", 600, "analis");
    assert_eq!(
        kirim(
            &r,
            get_dengan("/_uji/baca", Some(("authorization", &bearer(&ok))))
        )
        .await
        .0,
        StatusCode::OK
    );
    let iss_salah = jwt_buatan(&u.state.config, "titeny", "pihak-lain", 600, "analis");
    assert_eq!(
        kirim(
            &r,
            get_dengan("/_uji/baca", Some(("authorization", &bearer(&iss_salah))))
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    let lewat = jwt_buatan(&u.state.config, "titeny", "chefgenie-auth", -10, "analis");
    assert_eq!(
        kirim(
            &r,
            get_dengan("/_uji/baca", Some(("authorization", &bearer(&lewat))))
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    // tanda tangan salah
    let mut lain = Config::untuk_uji();
    lain.jwt_secret = "rahasia-lain-rahasia-lain-rahasia-lain-1".into();
    let palsu = jwt_buatan(&lain, "titeny", "chefgenie-auth", 600, "analis");
    assert_eq!(
        kirim(
            &r,
            get_dengan("/_uji/baca", Some(("authorization", &bearer(&palsu))))
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    // tanpa kredensial
    assert_eq!(
        kirim(&r, get_dengan("/_uji/baca", None)).await.0,
        StatusCode::UNAUTHORIZED
    );
}

#[sqlx::test]
async fn token_layanan_least_privilege(db: PgPool) {
    let u = siapkan(db);
    let r = router_uji(&u.state);
    let t = jwt_buatan(&u.state.config, "titeny", "chefgenie-auth", 600, "layanan");
    let h = bearer(&t);
    assert_eq!(
        kirim(&r, get_dengan("/_uji/layanan", Some(("authorization", &h))))
            .await
            .0,
        StatusCode::OK
    );
    let (st, _, b) = kirim(&r, get_dengan("/_uji/baca", Some(("authorization", &h)))).await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    assert_eq!(b["error"]["kode"], "tidak_berhak");
    assert_eq!(
        kirim(&r, get_dengan("/_uji/analis", Some(("authorization", &h))))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let ceo = bearer(&jwt_buatan(
        &u.state.config,
        "titeny",
        "chefgenie-auth",
        600,
        "ceo",
    ));
    assert_eq!(
        kirim(
            &r,
            get_dengan("/_uji/analis", Some(("authorization", &ceo)))
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        kirim(
            &r,
            get_dengan("/_uji/layanan", Some(("authorization", &ceo)))
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
}

#[sqlx::test]
async fn pin_zona_memberi_akses_baca_sesuai_zona(db: PgPool) {
    let u = siapkan(db.clone());
    let r = router_uji(&u.state);
    sisip_pin(&db, &u.state.config, "bekasi_utara", "vendor", "123456").await;
    let (st, _, b) = kirim(&r, get_dengan("/_uji/zona", Some(("x-zona-pin", "123456")))).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(b, "bekasi_utara");
    assert_eq!(
        kirim(&r, get_dengan("/_uji/baca", Some(("x-zona-pin", "123456"))))
            .await
            .0,
        StatusCode::OK
    );
    for salah in ["654321", "12345", "abcdef", "1234567"] {
        assert_eq!(
            kirim(&r, get_dengan("/_uji/baca", Some(("x-zona-pin", salah))))
                .await
                .0,
            StatusCode::UNAUTHORIZED,
            "{salah}"
        );
    }
    // PIN tidak boleh menembus endpoint staf/layanan.
    assert_eq!(
        kirim(
            &r,
            get_dengan("/_uji/analis", Some(("x-zona-pin", "123456")))
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    // PIN nonaktif ditolak.
    sqlx::query("UPDATE zona_pin SET aktif = FALSE")
        .execute(&db)
        .await
        .unwrap();
    assert_eq!(
        kirim(&r, get_dengan("/_uji/baca", Some(("x-zona-pin", "123456"))))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
}

#[sqlx::test]
async fn batas_laju_429_dengan_retry_after_per_token(db: PgPool) {
    let mut cfg = Config::untuk_uji();
    cfg.batas_per_menit = 3;
    let u = siapkan_dengan(db, cfg);
    let r = router_uji(&u.state);
    let a = bearer(&jwt_buatan(
        &u.state.config,
        "titeny",
        "chefgenie-auth",
        600,
        "ceo",
    ));
    let b = bearer(&jwt_buatan(
        &u.state.config,
        "titeny",
        "chefgenie-auth",
        600,
        "ceo",
    ));
    for _ in 0..3 {
        assert_eq!(
            kirim(&r, get_dengan("/_uji/baca", Some(("authorization", &a))))
                .await
                .0,
            StatusCode::OK
        );
    }
    let (st, h, body) = kirim(&r, get_dengan("/_uji/baca", Some(("authorization", &a)))).await;
    assert_eq!(st, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(body["error"]["kode"], "kuota_habis");
    let retry: u64 = h
        .get("retry-after")
        .unwrap()
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    assert!((1..=60).contains(&retry));
    // token lain tidak terpengaruh
    assert_eq!(
        kirim(&r, get_dengan("/_uji/baca", Some(("authorization", &b))))
            .await
            .0,
        StatusCode::OK
    );
}
