mod common;

use axum::{body::Body, http::Request, http::StatusCode};
use common::*;
use serde_json::{json, Value};
use sqlx::PgPool;
use titeny::auth::Peran;

fn amplop(n: u32, tipe: &str, source: &str, payload: Value) -> Value {
    json!({
        "event_id": format!("evt_20261004_{n:06}"), "event_type": tipe, "schema_version": 1,
        "source": source, "occurred_at": "2026-10-04T07:00:00+07:00",
        "received_at": "2026-10-04T07:00:41+07:00", "batch_id": "batch_uji", "payload": payload
    })
}

/// 11 fixture sah, satu per event_type terkunci.
fn fixtures() -> Vec<Value> {
    vec![
        amplop(
            1,
            "harga.diperbarui",
            "pasaree",
            json!({"pasar_id":"psr_jakarta_timur_01","komoditas_id":"cmd_cabai_merah","tanggal":"2026-10-04","harga_per_kg":42000,"satuan":"kg","jumlah_sampel_lapak":6,"flag":"ok"}),
        ),
        amplop(
            2,
            "stok.berubah",
            "lumbung",
            json!({"gudang_id":"gdg_bekasi_01","komoditas_id":"cmd_beras_medium","perubahan_kg":-150.50,"stok_akhir_kg":4850.00,"alasan":"keluar_penjualan"}),
        ),
        amplop(
            3,
            "stok.snapshot_harian",
            "lumbung",
            json!({"gudang_id":"gdg_bekasi_01","tanggal":"2026-10-03","baris":[{"komoditas_id":"cmd_beras_medium","stok_kg":4850.00}],"jumlah_baris":1}),
        ),
        amplop(
            4,
            "transaksi.dibuat",
            "titipo",
            json!({"transaksi_id":"trx_20261004_881201","komoditas_id":"cmd_telur_ayam","qty_kg":2.00,"harga_satuan":28500,"vendor_id":"vnd_keliling_014","zona_rute":"bekasi_utara","waktu":"2026-10-04T08:12:00+07:00"}),
        ),
        amplop(
            5,
            "lapak.status_berubah",
            "pasaree",
            json!({"lapak_id":"lpk_042","status":"aktif","komoditas_utama":"cmd_cabai_merah"}),
        ),
        amplop(
            6,
            "panen.dicatat",
            "lumbung",
            json!({"panen_id":"pn_20261001_0091","komoditas_id":"cmd_padi","lahan_ha":1.25,"hasil_kg":6875.00,"tanggal_panen":"2026-10-01","mutu":"A","kabupaten_id":"kab_karawang","keterangan":""}),
        ),
        amplop(
            7,
            "logistik.biaya_diperbarui",
            "titipo",
            json!({"rute_id":"rte_bekasi_jaktim","jarak_km":18.5,"biaya_per_kg":900,"moda":"mobil_pickup","berlaku_mulai":"2026-10-05","sebab":"penyesuaian_bbm"}),
        ),
        amplop(
            8,
            "vendor.status_berubah",
            "titipo",
            json!({"vendor_id":"vnd_keliling_014","jenis":"keliling","status_aktif":true,"zona_operasi":"bekasi_utara","kapasitas_harian_kg":120.0}),
        ),
        amplop(
            9,
            "cuaca.diperbarui",
            "cuaca",
            json!({"kabupaten_id":"kab_karawang","tanggal":"2026-10-04","curah_hujan_mm":12.5,"suhu_avg_c":28.4,"peringatan":"tidak_ada"}),
        ),
        amplop(
            10,
            "kalender.dikunci",
            "internal",
            json!({"tahun":2027,"baris":[{"tanggal":"2027-03-20","jenis_hari":"puasa","musim":"peralihan"}],"jumlah_baris":1,"dikunci_pada":"2026-12-01T00:00:00+07:00"}),
        ),
        amplop(
            11,
            "keuangan.ringkasan_harian",
            "lumbung",
            json!({"tanggal":"2026-10-03","omzet_idr":48250000,"margin_kotor_idr":7210000,"biaya_operasional_idr":4350000,"piutang_idr":1200000,"final":true}),
        ),
    ]
}

fn kirim_event(auth: &str, ev: &Value) -> Request<Body> {
    let id = ev["event_id"].as_str().unwrap_or("evt_x");
    Request::post("/api/v1/event/masuk")
        .header("content-type", "application/json")
        .header("authorization", auth)
        .header("x-idempotency-key", id)
        .body(Body::from(ev.to_string()))
        .unwrap()
}

async fn jumlah(db: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM event_masuk")
        .fetch_one(db)
        .await
        .unwrap()
}

#[sqlx::test]
async fn sebelas_fixture_sah_diterima(db: PgPool) {
    let u = siapkan(db.clone());
    let auth = token(&u.state, Peran::Layanan);
    let fx = fixtures();
    assert_eq!(fx.len(), 11);
    for ev in &fx {
        let (st, _, b) = kirim(&u.router, kirim_event(&auth, ev)).await;
        assert_eq!(st, StatusCode::OK, "{}: {b}", ev["event_type"]);
        assert_eq!(b, json!({"status":"diterima"}));
    }
    assert_eq!(jumlah(&db).await, 11);
}

#[sqlx::test]
async fn kirim_ulang_event_sama_dua_kali_hanya_satu_baris(db: PgPool) {
    let u = siapkan(db.clone());
    let auth = token(&u.state, Peran::Layanan);
    let ev = &fixtures()[0];
    let (_, _, b1) = kirim(&u.router, kirim_event(&auth, ev)).await;
    let (st, _, b2) = kirim(&u.router, kirim_event(&auth, ev)).await;
    assert_eq!(b1["status"], "diterima");
    assert_eq!(st, StatusCode::OK);
    assert_eq!(b2, json!({"status":"duplikat_diabaikan"}));
    assert_eq!(jumlah(&db).await, 1);
}

#[sqlx::test]
async fn kunci_idempoten_per_tipe_menangkap_event_id_berbeda(db: PgPool) {
    let u = siapkan(db.clone());
    let auth = token(&u.state, Peran::Layanan);
    let fx = fixtures();
    // harga: pasar_id + komoditas_id + tanggal sama, event_id beda => duplikat.
    let mut kedua = fx[0].clone();
    kedua["event_id"] = json!("evt_20261004_000999");
    kedua["payload"]["harga_per_kg"] = json!(43000);
    kirim(&u.router, kirim_event(&auth, &fx[0])).await;
    let (_, _, b) = kirim(&u.router, kirim_event(&auth, &kedua)).await;
    assert_eq!(b["status"], "duplikat_diabaikan");
    // transaksi: transaksi_id sama => duplikat.
    let mut trx = fx[3].clone();
    kirim(&u.router, kirim_event(&auth, &trx)).await;
    trx["event_id"] = json!("evt_20261004_000998");
    let (_, _, b) = kirim(&u.router, kirim_event(&auth, &trx)).await;
    assert_eq!(b["status"], "duplikat_diabaikan");
    // stok.berubah: kuncinya event_id, jadi event_id beda = baris baru.
    let mut stok = fx[1].clone();
    kirim(&u.router, kirim_event(&auth, &stok)).await;
    stok["event_id"] = json!("evt_20261004_000997");
    let (_, _, b) = kirim(&u.router, kirim_event(&auth, &stok)).await;
    assert_eq!(b["status"], "diterima");
    assert_eq!(jumlah(&db).await, 4); // harga 1 + transaksi 1 + stok 2
}

async fn ditolak(u: &Uji, ev: Value) -> Value {
    let auth = token(&u.state, Peran::Layanan);
    let (st, _, b) = kirim(&u.router, kirim_event(&auth, &ev)).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{ev} => {b}");
    assert_eq!(b["status"], "ditolak");
    b
}

#[sqlx::test]
async fn event_type_di_luar_11_ditolak(db: PgPool) {
    let u = siapkan(db.clone());
    for tipe in ["pedaree.stok.v1", "harga.dihapus", "HARGA.DIPERBARUI", ""] {
        let mut ev = fixtures()[0].clone();
        ev["event_type"] = json!(tipe);
        let b = ditolak(&u, ev).await;
        assert_eq!(b["alasan"], "event_type_tidak_dikenal", "{tipe}");
        assert_eq!(b["field"], "event_type");
    }
    assert_eq!(jumlah(&db).await, 0);
}

#[sqlx::test]
async fn validasi_amplop_dan_payload_berkode_plus_field(db: PgPool) {
    let u = siapkan(db.clone());
    let fx = fixtures();
    let kasus: Vec<(Value, &str)> = vec![
        // amplop
        (
            {
                let mut e = fx[0].clone();
                e["event_id"] = json!("evt_2026_1");
                e
            },
            "event_id",
        ),
        (
            {
                let mut e = fx[0].clone();
                e["schema_version"] = json!(0);
                e
            },
            "schema_version",
        ),
        (
            {
                let mut e = fx[0].clone();
                e["source"] = json!("gojek");
                e
            },
            "source",
        ),
        (
            {
                let mut e = fx[0].clone();
                e["occurred_at"] = json!("kemarin");
                e
            },
            "occurred_at",
        ),
        (
            {
                let mut e = fx[0].clone();
                e.as_object_mut().unwrap().remove("batch_id");
                e
            },
            "batch_id",
        ),
        (
            {
                let mut e = fx[0].clone();
                e.as_object_mut().unwrap().remove("payload");
                e
            },
            "payload",
        ),
        // harga
        (
            {
                let mut e = fx[0].clone();
                e["payload"]["harga_per_kg"] = json!(0);
                e
            },
            "payload.harga_per_kg",
        ),
        (
            {
                let mut e = fx[0].clone();
                e["payload"]["jumlah_sampel_lapak"] = json!(2);
                e
            },
            "payload.jumlah_sampel_lapak",
        ),
        (
            {
                let mut e = fx[0].clone();
                e["payload"]["satuan"] = json!("ton");
                e
            },
            "payload.satuan",
        ),
        (
            {
                let mut e = fx[0].clone();
                e["payload"].as_object_mut().unwrap().remove("tanggal");
                e
            },
            "payload.tanggal",
        ),
        // stok tidak negatif
        (
            {
                let mut e = fx[1].clone();
                e["payload"]["stok_akhir_kg"] = json!(-1.0);
                e
            },
            "payload.stok_akhir_kg",
        ),
        // qty maks 50 kg
        (
            {
                let mut e = fx[3].clone();
                e["payload"]["qty_kg"] = json!(50.01);
                e
            },
            "payload.qty_kg",
        ),
        // hasil per ha di luar 500-15.000 tanpa keterangan
        (
            {
                let mut e = fx[5].clone();
                e["payload"]["hasil_kg"] = json!(100.0);
                e
            },
            "payload.hasil_kg",
        ),
        (
            {
                let mut e = fx[5].clone();
                e["payload"]["hasil_kg"] = json!(30000.0);
                e
            },
            "payload.hasil_kg",
        ),
        // enum
        (
            {
                let mut e = fx[4].clone();
                e["payload"]["status"] = json!("rusak");
                e
            },
            "payload.status",
        ),
        (
            {
                let mut e = fx[8].clone();
                e["payload"]["peringatan"] = json!("gempa");
                e
            },
            "payload.peringatan",
        ),
        (
            {
                let mut e = fx[9].clone();
                e["payload"]["tahun"] = json!(2025);
                e
            },
            "payload.tahun",
        ),
        (
            {
                let mut e = fx[10].clone();
                e["payload"]["omzet_idr"] = json!(-5);
                e
            },
            "payload.omzet_idr",
        ),
    ];
    for (ev, field) in kasus {
        let b = ditolak(&u, ev).await;
        assert_eq!(b["alasan"], "validasi_gagal");
        assert!(
            b["field"].as_str().unwrap().starts_with(field),
            "field {} bukan {field}",
            b["field"]
        );
    }
    assert_eq!(jumlah(&db).await, 0);
}

#[sqlx::test]
async fn sampel_kurang_dengan_flag_dan_panen_outlier_berketerangan_diterima_bertanda(db: PgPool) {
    let u = siapkan(db.clone());
    let auth = token(&u.state, Peran::Layanan);
    let fx = fixtures();
    let mut h = fx[0].clone();
    h["payload"]["jumlah_sampel_lapak"] = json!(2);
    h["payload"]["flag"] = json!("perlu_verifikasi");
    let mut p = fx[5].clone();
    p["payload"]["hasil_kg"] = json!(30000.0);
    p["payload"]["keterangan"] = json!("panen raya lahan subur");
    for ev in [&h, &p, &fx[1]] {
        assert_eq!(
            kirim(&u.router, kirim_event(&auth, ev)).await.0,
            StatusCode::OK
        );
    }
    let bertanda: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM event_masuk WHERE flag = 'perlu_verifikasi'")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(bertanda, 2);
}

#[sqlx::test]
async fn header_idempotensi_wajib_dan_harus_sama_dengan_event_id(db: PgPool) {
    let u = siapkan(db.clone());
    let auth = token(&u.state, Peran::Layanan);
    let ev = &fixtures()[0];
    let tanpa = Request::post("/api/v1/event/masuk")
        .header("authorization", &auth)
        .body(Body::from(ev.to_string()))
        .unwrap();
    assert_eq!(kirim(&u.router, tanpa).await.0, StatusCode::BAD_REQUEST);
    let beda = Request::post("/api/v1/event/masuk")
        .header("authorization", &auth)
        .header("x-idempotency-key", "evt_20261004_999999")
        .body(Body::from(ev.to_string()))
        .unwrap();
    let (st, _, b) = kirim(&u.router, beda).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(b["field"], "X-Idempotency-Key");
    assert_eq!(jumlah(&db).await, 0);
}

#[sqlx::test]
async fn hanya_token_layanan_yang_boleh_menulis(db: PgPool) {
    let u = siapkan(db.clone());
    let ev = &fixtures()[0];
    for peran in [Peran::Ceo, Peran::Analis] {
        let (st, _, b) = kirim(&u.router, kirim_event(&token(&u.state, peran), ev)).await;
        assert_eq!(st, StatusCode::FORBIDDEN);
        assert_eq!(b["error"]["kode"], "tidak_berhak");
    }
    let tanpa = Request::post("/api/v1/event/masuk")
        .header("x-idempotency-key", "evt_20261004_000001")
        .body(Body::from(ev.to_string()))
        .unwrap();
    assert_eq!(kirim(&u.router, tanpa).await.0, StatusCode::UNAUTHORIZED);
    let pin = Request::post("/api/v1/event/masuk")
        .header("x-zona-pin", "123456")
        .header("x-idempotency-key", "evt_20261004_000001")
        .body(Body::from(ev.to_string()))
        .unwrap();
    assert_eq!(kirim(&u.router, pin).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(jumlah(&db).await, 0);
}

#[sqlx::test]
async fn bodi_rusak_400_dan_bodi_di_atas_256kb_413(db: PgPool) {
    let u = siapkan(db);
    let auth = token(&u.state, Peran::Layanan);
    let rusak = Request::post("/api/v1/event/masuk")
        .header("authorization", &auth)
        .header("x-idempotency-key", "evt_20261004_000001")
        .body(Body::from("{rusak"))
        .unwrap();
    assert_eq!(kirim(&u.router, rusak).await.0, StatusCode::BAD_REQUEST);
    let besar = Request::post("/api/v1/event/masuk")
        .header("authorization", &auth)
        .header("x-idempotency-key", "evt_20261004_000001")
        .body(Body::from(vec![b'a'; 256 * 1024 + 1]))
        .unwrap();
    assert_eq!(
        kirim(&u.router, besar).await.0,
        StatusCode::PAYLOAD_TOO_LARGE
    );
}
