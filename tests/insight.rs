mod common;

use axum::http::StatusCode;
use chrono::{Duration, NaiveDate, Utc};
use common::*;
use serde_json::Value;
use sqlx::PgPool;
use titeny::auth::Peran;

fn hari_ini() -> NaiveDate {
    (Utc::now() + Duration::hours(7)).date_naive()
}

async fn seed_harga(db: &PgPool, komoditas: &str, pasar: &str, hari: i64) {
    let t = hari_ini() + Duration::days(hari);
    sqlx::query(
        "INSERT INTO prediksi_harga VALUES ($1,$2,$3,$4,$5,$6,$7,'sedang','stok_gudang_72%',$8,NOW())",
    )
    .bind(komoditas)
    .bind(pasar)
    .bind(t)
    .bind(format!("H+{}", hari.min(7)))
    .bind(42500 + (hari as i32) * 500)
    .bind(39000)
    .bind(46000)
    .bind("harga_v1.0.0")
    .execute(db)
    .await
    .unwrap();
}

async fn seed_panen(db: &PgPool, komoditas: &str, kab: &str, minggu: &str) {
    sqlx::query("INSERT INTO prediksi_panen VALUES ($1,$2,$3,12500,11000,14000,'tinggi','panen_v1.0.0',NOW())")
        .bind(komoditas)
        .bind(kab)
        .bind(minggu)
        .execute(db)
        .await
        .unwrap();
}

async fn seed_rek(db: &PgPool, id: &str, zona: &str) {
    sqlx::query("INSERT INTO rekomendasi (id, zona, teks) VALUES ($1, $2, '[\"Amankan pasokan 800 kg\"]'::jsonb)")
        .bind(id)
        .bind(zona)
        .execute(db)
        .await
        .unwrap();
}

async fn get(
    u: &Uji,
    path: &str,
    auth: Option<(&str, &str)>,
) -> (StatusCode, axum::http::HeaderMap, Value) {
    kirim(&u.router, get_dengan(path, auth)).await
}

#[sqlx::test]
async fn harga_bentuk_respons_cache_dan_rentang(db: PgPool) {
    let u = siapkan(db.clone());
    for h in 1..=7 {
        seed_harga(&db, "cmd_cabai_merah", "psr_jakarta_timur_01", h).await;
    }
    seed_harga(&db, "cmd_cabai_merah", "psr_jakarta_timur_01", 9).await; // di atas H+7 tidak tampil
    let t = token(&u.state, Peran::Ceo);
    let (st, h, b) = get(
        &u,
        "/api/v1/harga?komoditas_id=cmd_cabai_merah&pasar_id=psr_jakarta_timur_01",
        Some(("authorization", &t)),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{b}");
    assert_eq!(h["cache-control"], "public, max-age=21600");
    assert!(h["vary"].to_str().unwrap().contains("Authorization"));
    assert_eq!(b["model_version"], "harga_v1.0.0");
    assert!(b["dihitung_pada"].as_str().unwrap().ends_with("+07:00"));
    let deret = b["deret"].as_array().unwrap();
    assert_eq!(deret.len(), 7, "H+9 tidak boleh tampil");
    assert_eq!(deret[0]["horizon"], "H+1");
    assert_eq!(deret[0]["harga_prediksi"], 43000);
    assert_eq!(deret[0]["rentang_bawah"], 39000);
    assert_eq!(deret[0]["rentang_atas"], 46000);
    assert_eq!(deret[0]["kepercayaan"], "sedang");
    assert_eq!(deret[0]["alasan_singkat"], "stok_gudang_72%");
    assert_eq!(
        deret[0]["tanggal"],
        (hari_ini() + Duration::days(1)).to_string()
    );
}

#[sqlx::test]
async fn harga_validasi_dan_galat_baku(db: PgPool) {
    let u = siapkan(db.clone());
    seed_harga(&db, "cmd_cabai_merah", "psr_jakarta_timur_01", 1).await;
    seed_harga(&db, "cmd_telur_ayam", "psr_bekasi_01", 1).await;
    let t = token(&u.state, Peran::Analis);
    let a = Some(("authorization", t.as_str()));
    let besok9 = hari_ini() + Duration::days(8);
    let kasus = [
        ("/api/v1/harga?pasar_id=psr_jakarta_timur_01".to_string(), 422, "validasi_gagal"),
        ("/api/v1/harga?komoditas_id=cmd_cabai_merah".to_string(), 422, "validasi_gagal"),
        ("/api/v1/harga?komoditas_id=CABAI&pasar_id=psr_jakarta_timur_01".to_string(), 422, "validasi_gagal"),
        (format!("/api/v1/harga?komoditas_id=cmd_cabai_merah&pasar_id=psr_jakarta_timur_01&sampai={besok9}"), 422, "validasi_gagal"),
        ("/api/v1/harga?komoditas_id=cmd_cabai_merah&pasar_id=psr_jakarta_timur_01&dari=bukan-tanggal".to_string(), 400, "validasi_gagal"),
        ("/api/v1/harga?komoditas_id=cmd_tidak_ada&pasar_id=psr_jakarta_timur_01".to_string(), 404, "tidak_ditemukan"),
        ("/api/v1/harga?komoditas_id=cmd_cabai_merah&pasar_id=psr_tidak_ada".to_string(), 404, "tidak_ditemukan"),
        // komoditas dan pasar ada, tetapi bukan pasangan ini: data belum cukup
        ("/api/v1/harga?komoditas_id=cmd_cabai_merah&pasar_id=psr_bekasi_01".to_string(), 404, "data_belum_cukup"),
    ];
    for (path, status, kode) in kasus {
        let (st, _, b) = get(&u, &path, a).await;
        assert_eq!(st.as_u16(), status, "{path} => {b}");
        assert_eq!(b["error"]["kode"], kode, "{path}");
    }
}

#[sqlx::test]
async fn pin_zona_hanya_membaca_zonanya_dan_layanan_ditolak(db: PgPool) {
    let u = siapkan(db.clone());
    seed_harga(&db, "cmd_cabai_merah", "psr_jakarta_timur_01", 1).await;
    seed_harga(&db, "cmd_cabai_merah", "psr_bekasi_01", 1).await;
    sisip_pin(
        &db,
        &u.state.config,
        "psr_jakarta_timur_01",
        "vendor",
        "123456",
    )
    .await;
    let pin = Some(("x-zona-pin", "123456"));
    // tanpa pasar_id: default zona sendiri
    let (st, _, b) = get(&u, "/api/v1/harga?komoditas_id=cmd_cabai_merah", pin).await;
    assert_eq!(st, StatusCode::OK, "{b}");
    assert_eq!(b["pasar_id"], "psr_jakarta_timur_01");
    // zona lain ditolak
    let (st, _, b) = get(
        &u,
        "/api/v1/harga?komoditas_id=cmd_cabai_merah&pasar_id=psr_bekasi_01",
        pin,
    )
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    assert_eq!(b["error"]["kode"], "tidak_berhak");
    // token layanan tidak boleh membaca
    let t = token(&u.state, Peran::Layanan);
    let (st, _, _) = get(
        &u,
        "/api/v1/harga?komoditas_id=cmd_cabai_merah&pasar_id=psr_bekasi_01",
        Some(("authorization", &t)),
    )
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    // tanpa kredensial
    let (st, _, _) = get(
        &u,
        "/api/v1/harga?komoditas_id=cmd_cabai_merah&pasar_id=psr_bekasi_01",
        None,
    )
    .await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn demand_bentuk_cache_dan_galat(db: PgPool) {
    let u = siapkan(db.clone());
    sqlx::query(
        "INSERT INTO prediksi_demand VALUES ('cmd_cabai_merah','bekasi_utara','H+7_total_kg',$1,840.0,700.0,980.0,'defisit',
         '[\"Amankan pasokan dari Brebes\"]'::jsonb,'demand_v1.0.0',NOW())",
    )
    .bind(hari_ini())
    .execute(&db)
    .await
    .unwrap();
    let t = token(&u.state, Peran::Ceo);
    let a = Some(("authorization", t.as_str()));
    let (st, h, b) = get(
        &u,
        "/api/v1/demand?komoditas_id=cmd_cabai_merah&zona=bekasi_utara",
        a,
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{b}");
    assert_eq!(h["cache-control"], "public, max-age=21600");
    assert_eq!(b["jenis"], "demand");
    assert_eq!(b["horizon"], "H+7_total_kg");
    assert_eq!(b["demand_prediksi_kg"], 840.0);
    assert_eq!(b["sinyal"], "defisit");
    assert_eq!(b["model_version"], "demand_v1.0.0");
    assert_eq!(b["rekomendasi"][0], "Amankan pasokan dari Brebes");
    let (st, _, b) = get(
        &u,
        "/api/v1/demand?komoditas_id=cmd_cabai_merah&zona=bekasi_utara&horizon=H%2B30",
        a,
    )
    .await;
    assert_eq!(
        (st, b["error"]["kode"].as_str()),
        (StatusCode::NOT_FOUND, Some("data_belum_cukup"))
    );
    let (st, _, _) = get(
        &u,
        "/api/v1/demand?komoditas_id=cmd_cabai_merah&zona=bekasi_utara&horizon=H%2B3",
        a,
    )
    .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    let (st, _, b) = get(
        &u,
        "/api/v1/demand?komoditas_id=cmd_kangkung&zona=bekasi_utara",
        a,
    )
    .await;
    assert_eq!(
        (st, b["error"]["kode"].as_str()),
        (StatusCode::NOT_FOUND, Some("tidak_ditemukan"))
    );
}

#[sqlx::test]
async fn panen_paginasi_cursor_opaque_dan_batas_limit(db: PgPool) {
    let u = siapkan(db.clone());
    for (k, kab) in [
        ("cmd_padi", "kab_karawang"),
        ("cmd_padi", "kab_subang"),
        ("cmd_jagung", "kab_garut"),
    ] {
        for m in ["2026-W41", "2026-W42"] {
            seed_panen(&db, k, kab, m).await;
        }
    }
    let t = token(&u.state, Peran::Ceo);
    let a = Some(("authorization", t.as_str()));
    let mut semua = vec![];
    let mut path = "/api/v1/panen?limit=4".to_string();
    let mut halaman = 0;
    loop {
        let (st, h, b) = get(&u, &path, a).await;
        assert_eq!(st, StatusCode::OK, "{b}");
        assert_eq!(h["cache-control"], "public, max-age=21600");
        halaman += 1;
        for r in b["deret"].as_array().unwrap() {
            assert_eq!(r["jenis"], "panen");
            assert_eq!(r["model_version"], "panen_v1.0.0");
            semua.push(format!(
                "{}|{}|{}",
                r["komoditas_id"], r["kabupaten_id"], r["minggu"]
            ));
        }
        match b["next_cursor"].as_str() {
            Some(c) => path = format!("/api/v1/panen?limit=4&cursor={c}"),
            None => break,
        }
    }
    assert_eq!(halaman, 2);
    assert_eq!(semua.len(), 6);
    let mut unik = semua.clone();
    unik.sort();
    unik.dedup();
    assert_eq!(unik.len(), 6, "tidak boleh ada duplikat antarhalaman");
    // limit default 50 sah; di luar 1-200 dan cursor rusak ditolak
    for (p, kode) in [
        ("limit=0", 422),
        ("limit=201", 422),
        ("cursor=%21%21", 422),
        ("limit=abc", 400),
        ("minggu=2026-41", 422),
    ] {
        let (st, _, b) = get(&u, &format!("/api/v1/panen?{p}"), a).await;
        assert_eq!(st.as_u16(), kode, "{p} => {b}");
    }
    let (st, _, b) = get(
        &u,
        "/api/v1/panen?kabupaten_id=kab_garut&komoditas_id=cmd_jagung&minggu=2026-W42",
        a,
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(b["deret"].as_array().unwrap().len(), 1);
    let (st, _, b) = get(&u, "/api/v1/panen?kabupaten_id=kab_malang", a).await;
    assert_eq!(
        (st, b["error"]["kode"].as_str()),
        (StatusCode::NOT_FOUND, Some("data_belum_cukup"))
    );
}

#[sqlx::test]
async fn rekomendasi_paginasi_filter_zona_dan_pin(db: PgPool) {
    let u = siapkan(db.clone());
    for i in 1..=5 {
        seed_rek(
            &db,
            &format!("rek_{i:03}"),
            if i % 2 == 0 {
                "bekasi_utara"
            } else {
                "bekasi_selatan"
            },
        )
        .await;
    }
    let t = token(&u.state, Peran::Ceo);
    let a = Some(("authorization", t.as_str()));
    let (st, h, b) = get(&u, "/api/v1/rekomendasi?limit=2", a).await;
    assert_eq!(st, StatusCode::OK);
    assert!(
        h.get("cache-control").is_none(),
        "rekomendasi berstatus, tidak di-cache publik"
    );
    assert_eq!(b["data"].as_array().unwrap().len(), 2);
    assert_eq!(b["data"][0]["id"], "rek_005", "terbaru dulu");
    assert_eq!(b["data"][0]["status"], "baru");
    let c = b["next_cursor"].as_str().unwrap();
    let (_, _, b2) = get(&u, &format!("/api/v1/rekomendasi?limit=2&cursor={c}"), a).await;
    assert_eq!(b2["data"][0]["id"], "rek_003");
    let (_, _, bz) = get(&u, "/api/v1/rekomendasi?zona=bekasi_utara", a).await;
    assert_eq!(bz["data"].as_array().unwrap().len(), 2);
    assert!(bz["next_cursor"].is_null());
    sisip_pin(&db, &u.state.config, "bekasi_utara", "vendor", "654321").await;
    let (st, _, bp) = get(&u, "/api/v1/rekomendasi", Some(("x-zona-pin", "654321"))).await;
    assert_eq!(st, StatusCode::OK);
    assert!(bp["data"]
        .as_array()
        .unwrap()
        .iter()
        .all(|r| r["zona"] == "bekasi_utara"));
    let (st, _, _) = get(
        &u,
        "/api/v1/rekomendasi?zona=bekasi_selatan",
        Some(("x-zona-pin", "654321")),
    )
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN);
}

#[sqlx::test]
async fn kesehatan_publik_cache_30_detik_dan_alias(db: PgPool) {
    let u = siapkan(db.clone());
    for path in ["/api/v1/kesehatan", "/v1/kesehatan"] {
        let (st, h, b) = get(&u, path, None).await;
        assert_eq!(st, StatusCode::OK, "{path}");
        assert_eq!(h["cache-control"], "public, max-age=30");
        assert_eq!(b["status"], "ok");
        assert_eq!(b["versi_api"], "v1.0.0");
        assert!(b["job_terakhir"].is_null(), "belum ada job");
    }
    seed_harga(&db, "cmd_cabai_merah", "psr_jakarta_timur_01", 1).await;
    let (_, _, b) = get(&u, "/api/v1/kesehatan", None).await;
    assert!(b["job_terakhir"].as_str().unwrap().ends_with("+07:00"));
}

#[sqlx::test]
async fn auth_wajib_di_semua_endpoint_baca_selain_kesehatan(db: PgPool) {
    let u = siapkan(db);
    for p in [
        "/api/v1/harga",
        "/api/v1/demand",
        "/api/v1/panen",
        "/api/v1/rekomendasi",
    ] {
        let (st, _, b) = get(&u, p, None).await;
        assert_eq!(st, StatusCode::UNAUTHORIZED, "{p}");
        assert_eq!(b["error"]["kode"], "tidak_berhak");
    }
}
