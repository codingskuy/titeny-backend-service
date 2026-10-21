mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use chrono::{Duration, NaiveDate, Utc};
use common::*;
use serde_json::{json, Value};
use sqlx::PgPool;
use titeny::auth::Peran;

fn hari_ini() -> NaiveDate {
    (Utc::now() + Duration::hours(7)).date_naive()
}

/// Masukkan event harga lewat endpoint ingest. `bertanda` = sampel < 3 + flag perlu_verifikasi.
async fn seed_event(u: &Uji, n: u32, bertanda: bool) -> String {
    let id = format!("evt_20261004_{n:06}");
    let ev = json!({
        "event_id": id, "event_type": "harga.diperbarui", "schema_version": 1, "source": "pasaree",
        "occurred_at": "2026-10-04T07:00:00+07:00", "received_at": "2026-10-04T07:00:41+07:00",
        "batch_id": "b", "payload": {
            "pasar_id": format!("psr_uji_{n:03}"), "komoditas_id": "cmd_cabai_merah", "tanggal": "2026-10-04",
            "harga_per_kg": 42000, "satuan": "kg",
            "jumlah_sampel_lapak": if bertanda { 2 } else { 6 },
            "flag": if bertanda { "perlu_verifikasi" } else { "ok" }
        }
    });
    let req = Request::post("/api/v1/event/masuk")
        .header("authorization", token(&u.state, Peran::Layanan))
        .header("x-idempotency-key", &id)
        .body(Body::from(ev.to_string()))
        .unwrap();
    let (st, _, b) = kirim(&u.router, req).await;
    assert_eq!(st, StatusCode::OK, "{b}");
    id
}

fn aksi(auth: &str, path: &str, kunci: &str, body: Value) -> Request<Body> {
    Request::post(path)
        .header("content-type", "application/json")
        .header("authorization", auth)
        .header("x-idempotency-key", kunci)
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn validasi_body(aksi_id: &str, event_id: &str) -> Value {
    json!({"aksi_id": aksi_id, "event_id": event_id, "keputusan": "setuju", "alasan": "Nota lapak sesuai harga."})
}

async fn hitung(db: &PgPool, tabel: &str) -> i64 {
    sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {tabel}"))
        .fetch_one(db)
        .await
        .unwrap()
}

#[sqlx::test]
async fn validasi_tercatat_dengan_audit_siapa_kapan_apa_alasan(db: PgPool) {
    let u = siapkan(db.clone());
    let ev = seed_event(&u, 1, true).await;
    let t = token(&u.state, Peran::Analis);
    let body = json!({"aksi_id":"aks_001","event_id":ev,"keputusan":"koreksi_nilai","nilai_baru":41000,"alasan":"Salah input lapak 042, nota terlampir."});
    let (st, _, b) = kirim(
        &u.router,
        aksi(&t, "/api/v1/analis/validasi", "aks_001", body),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{b}");
    assert_eq!(b, json!({"status":"diterima"}));
    let (keputusan, nilai): (String, Option<i64>) =
        sqlx::query_as("SELECT keputusan, nilai_baru FROM validasi_analis WHERE aksi_id='aks_001'")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!((keputusan.as_str(), nilai), ("koreksi_nilai", Some(41000)));
    let (pelaku, alasan, apa, retensi_hari): (String, String, Value, i32) = sqlx::query_as(
        "SELECT pelaku, alasan, apa, (retensi_sampai::date - waktu::date)::int FROM audit_aksi WHERE aksi_id='aks_001'",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert!(!pelaku.is_empty());
    assert_eq!(alasan, "Salah input lapak 042, nota terlampir.");
    assert_eq!(apa["jenis"], "validasi");
    assert_eq!(apa["event_id"], ev);
    assert!(
        (729..=731).contains(&retensi_hari),
        "retensi 2 tahun, dapat {retensi_hari}"
    );
    // angka event sumber tidak berubah
    let harga: Value =
        sqlx::query_scalar("SELECT payload->'harga_per_kg' FROM event_masuk WHERE event_id=$1")
            .bind(&ev)
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(harga, json!(42000));
}

#[sqlx::test]
async fn kirim_ulang_aksi_id_sama_200_duplikat_tanpa_rekaman_ganda(db: PgPool) {
    let u = siapkan(db.clone());
    let ev = seed_event(&u, 1, true).await;
    let t = token(&u.state, Peran::Analis);
    let (_, _, b1) = kirim(
        &u.router,
        aksi(
            &t,
            "/api/v1/analis/validasi",
            "aks_001",
            validasi_body("aks_001", &ev),
        ),
    )
    .await;
    let (st, _, b2) = kirim(
        &u.router,
        aksi(
            &t,
            "/api/v1/analis/validasi",
            "aks_001",
            validasi_body("aks_001", &ev),
        ),
    )
    .await;
    assert_eq!(b1["status"], "diterima");
    assert_eq!(st, StatusCode::OK);
    assert_eq!(b2, json!({"status":"duplikat_diabaikan"}));
    assert_eq!(hitung(&db, "validasi_analis").await, 1);
    assert_eq!(hitung(&db, "audit_aksi").await, 1);

    let tahan = json!({"aksi_id":"aks_002","komoditas_id":"cmd_cabai_merah","pasar_id":"psr_jakarta_timur_01",
        "alasan":"Sampel hanya 2 lapak.","berlaku_sampai": hari_ini().to_string()});
    for _ in 0..2 {
        let (st, _, _) = kirim(
            &u.router,
            aksi(&t, "/api/v1/analis/tahan", "aks_002", tahan.clone()),
        )
        .await;
        assert_eq!(st, StatusCode::OK);
    }
    assert_eq!(hitung(&db, "tahan_prediksi").await, 1);
    assert_eq!(hitung(&db, "audit_aksi").await, 2);
}

#[sqlx::test]
async fn validasi_aturan_masukan(db: PgPool) {
    let u = siapkan(db.clone());
    let ev = seed_event(&u, 1, true).await;
    let t = token(&u.state, Peran::Analis);
    let dasar = |m: Value| {
        let mut b = validasi_body("aks_x", &ev);
        for (k, v) in m.as_object().unwrap() {
            b[k] = v.clone();
        }
        b
    };
    let kasus = [
        (dasar(json!({"alasan": "pendek"})), "alasan"),
        (dasar(json!({"alasan": "a".repeat(141)})), "alasan"),
        (dasar(json!({"keputusan": "koreksi_nilai"})), "nilai_baru"),
        (
            dasar(json!({"keputusan": "koreksi_nilai", "nilai_baru": 0})),
            "nilai_baru",
        ),
        (dasar(json!({"nilai_baru": 100})), "nilai_baru"),
        (dasar(json!({"keputusan": "abaikan"})), "keputusan"),
        (dasar(json!({"event_id": "evt_salah"})), "event_id"),
        (dasar(json!({"aksi_id": ""})), "aksi_id"),
    ];
    for (body, field) in kasus {
        let kunci = body["aksi_id"].as_str().unwrap_or("aks_x").to_string();
        let (st, _, b) = kirim(
            &u.router,
            aksi(&t, "/api/v1/analis/validasi", &kunci, body.clone()),
        )
        .await;
        // aksi_id kosong tak bisa jadi header sah: wajib 400; lainnya 422 dengan field.
        if field == "aksi_id" {
            assert_eq!(st, StatusCode::BAD_REQUEST);
            continue;
        }
        assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{body} => {b}");
        assert_eq!(b["error"]["kode"], "validasi_gagal");
        assert_eq!(b["error"]["field"], field, "{body}");
    }
    assert_eq!(
        hitung(&db, "audit_aksi").await,
        0,
        "penolakan tidak boleh meninggalkan jejak aksi"
    );
    // batas 10 dan 140 karakter sah
    for (i, alasan) in ["sepuluh ok".to_string(), "b".repeat(140)]
        .into_iter()
        .enumerate()
    {
        let e = seed_event(&u, 10 + i as u32, true).await;
        let id = format!("aks_ok{i}");
        let (st, _, b) = kirim(
            &u.router,
            aksi(
                &t,
                "/api/v1/analis/validasi",
                &id,
                json!({"aksi_id": id, "event_id": e, "keputusan": "tolak", "alasan": alasan}),
            ),
        )
        .await;
        assert_eq!(st, StatusCode::OK, "{b}");
    }
}

#[sqlx::test]
async fn validasi_event_tak_ada_tak_bertanda_sudah_diputuskan_dan_aksi_id_silang(db: PgPool) {
    let u = siapkan(db.clone());
    let bertanda = seed_event(&u, 1, true).await;
    let biasa = seed_event(&u, 2, false).await;
    let t = token(&u.state, Peran::Analis);
    let (st, _, b) = kirim(
        &u.router,
        aksi(
            &t,
            "/api/v1/analis/validasi",
            "aks_1",
            validasi_body("aks_1", "evt_20261004_999999"),
        ),
    )
    .await;
    assert_eq!(
        (st, b["error"]["kode"].as_str()),
        (StatusCode::NOT_FOUND, Some("tidak_ditemukan"))
    );
    let (st, _, _) = kirim(
        &u.router,
        aksi(
            &t,
            "/api/v1/analis/validasi",
            "aks_2",
            validasi_body("aks_2", &biasa),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT);
    assert_eq!(hitung(&db, "audit_aksi").await, 0);

    let (st, _, _) = kirim(
        &u.router,
        aksi(
            &t,
            "/api/v1/analis/validasi",
            "aks_3",
            validasi_body("aks_3", &bertanda),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    // event yang sama dengan aksi_id lain: konflik, tidak ada rekaman/audit baru
    let (st, _, _) = kirim(
        &u.router,
        aksi(
            &t,
            "/api/v1/analis/validasi",
            "aks_4",
            validasi_body("aks_4", &bertanda),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT);
    assert_eq!(
        (
            hitung(&db, "validasi_analis").await,
            hitung(&db, "audit_aksi").await
        ),
        (1, 1)
    );
    // aksi_id yang sama dipakai untuk jenis aksi lain: konflik
    let tahan = json!({"aksi_id":"aks_3","komoditas_id":"cmd_cabai_merah","pasar_id":"psr_a","alasan":"Sampel hanya 2 lapak.","berlaku_sampai":hari_ini().to_string()});
    let (st, _, _) = kirim(&u.router, aksi(&t, "/api/v1/analis/tahan", "aks_3", tahan)).await;
    assert_eq!(st, StatusCode::CONFLICT);
}

#[sqlx::test]
async fn hanya_analis_dan_header_idempotensi(db: PgPool) {
    let u = siapkan(db.clone());
    let ev = seed_event(&u, 1, true).await;
    let b = validasi_body("aks_1", &ev);
    for peran in [Peran::Ceo, Peran::Layanan] {
        let (st, _, r) = kirim(
            &u.router,
            aksi(
                &token(&u.state, peran),
                "/api/v1/analis/validasi",
                "aks_1",
                b.clone(),
            ),
        )
        .await;
        assert_eq!(st, StatusCode::FORBIDDEN, "{r}");
    }
    let tanpa_token = Request::post("/api/v1/analis/validasi")
        .header("x-idempotency-key", "aks_1")
        .body(Body::from(b.to_string()))
        .unwrap();
    assert_eq!(
        kirim(&u.router, tanpa_token).await.0,
        StatusCode::UNAUTHORIZED
    );
    let t = token(&u.state, Peran::Analis);
    let tanpa_header = Request::post("/api/v1/analis/validasi")
        .header("authorization", &t)
        .body(Body::from(b.to_string()))
        .unwrap();
    assert_eq!(
        kirim(&u.router, tanpa_header).await.0,
        StatusCode::BAD_REQUEST
    );
    let (st, _, r) = kirim(
        &u.router,
        aksi(&t, "/api/v1/analis/validasi", "aks_lain", b.clone()),
    )
    .await;
    assert_eq!(
        (st, r["error"]["field"].as_str()),
        (StatusCode::UNPROCESSABLE_ENTITY, Some("X-Idempotency-Key"))
    );
    assert_eq!(hitung(&db, "audit_aksi").await, 0);
}

#[sqlx::test]
async fn tahan_maks_7_hari_dan_menyembunyikan_dari_papan_pin_saja(db: PgPool) {
    let u = siapkan(db.clone());
    sqlx::query("INSERT INTO prediksi_harga VALUES ('cmd_cabai_merah','psr_jakarta_timur_01',$1,'H+1',42500,39000,46000,'sedang',NULL,'harga_v1.0.0',NOW())")
        .bind(hari_ini() + Duration::days(1))
        .execute(&db)
        .await
        .unwrap();
    sisip_pin(
        &db,
        &u.state.config,
        "psr_jakarta_timur_01",
        "vendor",
        "123456",
    )
    .await;
    let pin = ("x-zona-pin", "123456");
    let staf = token(&u.state, Peran::Ceo);
    let analis = token(&u.state, Peran::Analis);
    let baca = |a: (&str, &str)| {
        get_dengan(
            "/api/v1/harga?komoditas_id=cmd_cabai_merah&pasar_id=psr_jakarta_timur_01",
            Some(a),
        )
    };
    assert_eq!(kirim(&u.router, baca(pin)).await.0, StatusCode::OK);

    let tahan = |id: &str, hari: i64| {
        json!({"aksi_id":id,"komoditas_id":"cmd_cabai_merah","pasar_id":"psr_jakarta_timur_01",
        "alasan":"Sampel hanya 2 lapak.","berlaku_sampai":(hari_ini()+Duration::days(hari)).to_string()})
    };
    for (id, hari) in [("aks_a", 8), ("aks_b", -1)] {
        let (st, _, b) = kirim(
            &u.router,
            aksi(&analis, "/api/v1/analis/tahan", id, tahan(id, hari)),
        )
        .await;
        assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "hari {hari}");
        assert_eq!(b["error"]["field"], "berlaku_sampai");
    }
    let (st, _, _) = kirim(
        &u.router,
        aksi(&analis, "/api/v1/analis/tahan", "aks_c", tahan("aks_c", 7)),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    // tersembunyi untuk PIN, tetap terlihat untuk CEO; angka tak berubah
    let (st, _, b) = kirim(&u.router, baca(pin)).await;
    assert_eq!(
        (st, b["error"]["kode"].as_str()),
        (StatusCode::NOT_FOUND, Some("data_belum_cukup"))
    );
    let (st, _, b) = kirim(&u.router, baca(("authorization", &staf))).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(b["deret"][0]["harga_prediksi"], 42500);
    // kedaluwarsa otomatis: penahanan lewat tanggal tidak berlaku lagi
    sqlx::query("UPDATE tahan_prediksi SET berlaku_sampai = $1")
        .bind(hari_ini() - Duration::days(1))
        .execute(&db)
        .await
        .unwrap();
    assert_eq!(kirim(&u.router, baca(pin)).await.0, StatusCode::OK);
}

#[sqlx::test]
async fn audit_tidak_dapat_diubah_atau_dihapus_selama_retensi(db: PgPool) {
    let u = siapkan(db.clone());
    let ev = seed_event(&u, 1, true).await;
    let t = token(&u.state, Peran::Analis);
    kirim(
        &u.router,
        aksi(
            &t,
            "/api/v1/analis/validasi",
            "aks_1",
            validasi_body("aks_1", &ev),
        ),
    )
    .await;
    for sql in [
        "DELETE FROM audit_aksi",
        "UPDATE audit_aksi SET alasan = 'diubah'",
        "TRUNCATE audit_aksi CASCADE",
    ] {
        assert!(
            sqlx::query(sql).execute(&db).await.is_err(),
            "{sql} harus ditolak"
        );
    }
    assert_eq!(hitung(&db, "audit_aksi").await, 1);
    // setelah masa retensi lewat, pembersihan terjadwal boleh menghapus; ubah tetap dilarang
    sqlx::query("INSERT INTO audit_aksi (aksi_id, jenis, pelaku, apa, alasan, retensi_sampai) VALUES ('aks_lama','tahan','x','{}','alasan lama',NOW() - INTERVAL '1 day')")
        .execute(&db).await.unwrap();
    assert!(
        sqlx::query("UPDATE audit_aksi SET alasan = 'x' WHERE aksi_id='aks_lama'")
            .execute(&db)
            .await
            .is_err()
    );
    assert_eq!(
        sqlx::query("DELETE FROM audit_aksi WHERE aksi_id='aks_lama'")
            .execute(&db)
            .await
            .unwrap()
            .rows_affected(),
        1
    );
}

#[sqlx::test]
async fn antrean_hanya_analis_bertanda_belum_diputuskan_dan_berpaginasi(db: PgPool) {
    let u = siapkan(db.clone());
    for n in 1..=3 {
        seed_event(&u, n, true).await;
    }
    seed_event(&u, 4, false).await;
    let t = token(&u.state, Peran::Analis);
    let a = Some(("authorization", t.as_str()));
    let (st, _, b) = kirim(&u.router, get_dengan("/api/v1/analis/antrean?limit=2", a)).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(b["data"].as_array().unwrap().len(), 2);
    assert_eq!(b["data"][0]["event_id"], "evt_20261004_000001");
    assert_eq!(b["data"][0]["flag"], "perlu_verifikasi");
    let c = b["next_cursor"].as_str().unwrap();
    let (_, _, b2) = kirim(
        &u.router,
        get_dengan(&format!("/api/v1/analis/antrean?limit=2&cursor={c}"), a),
    )
    .await;
    assert_eq!(b2["data"].as_array().unwrap().len(), 1);
    assert!(b2["next_cursor"].is_null());
    // yang sudah diputuskan keluar dari antrean
    kirim(
        &u.router,
        aksi(
            &t,
            "/api/v1/analis/validasi",
            "aks_1",
            validasi_body("aks_1", "evt_20261004_000001"),
        ),
    )
    .await;
    let (_, _, b3) = kirim(&u.router, get_dengan("/api/v1/analis/antrean", a)).await;
    assert_eq!(b3["data"].as_array().unwrap().len(), 2);
    let ceo = token(&u.state, Peran::Ceo);
    assert_eq!(
        kirim(
            &u.router,
            get_dengan("/api/v1/analis/antrean", Some(("authorization", &ceo)))
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        kirim(&u.router, get_dengan("/api/v1/analis/antrean", None))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
}
