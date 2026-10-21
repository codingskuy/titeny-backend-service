//! Pembatas laju: 60 permintaan/menit per token (429 + `Retry-After`).
//!
//! Jendela geser di memori (per instans). Kunci: hash token Bearer, atau
//! hash PIN zona, atau alamat klien untuk permintaan tanpa kredensial.

use std::{
    collections::{HashMap, VecDeque},
    net::SocketAddr,
    sync::Mutex,
    time::{Duration, Instant},
};

use axum::{
    extract::{ConnectInfo, Request, State},
    http::header,
    middleware::Next,
    response::{IntoResponse, Response},
};
use sha2::{Digest, Sha256};

use crate::{error::AppError, state::AppState};

const JENDELA: Duration = Duration::from_secs(60);
const BATAS_KUNCI_SAPU: usize = 10_000;

pub struct Pembatas {
    batas: u32,
    jendela: Duration,
    catatan: Mutex<HashMap<String, VecDeque<Instant>>>,
}

impl Pembatas {
    pub fn baru(batas: u32) -> Self {
        Self {
            batas,
            jendela: JENDELA,
            catatan: Mutex::new(HashMap::new()),
        }
    }

    /// `Ok(())` bila diizinkan; `Err(detik)` = nilai `Retry-After`.
    pub fn periksa(&self, kunci: &str) -> Result<(), u64> {
        self.periksa_pada(kunci, Instant::now())
    }

    fn periksa_pada(&self, kunci: &str, sekarang: Instant) -> Result<(), u64> {
        let mut peta = self.catatan.lock().unwrap_or_else(|e| e.into_inner());
        if peta.len() > BATAS_KUNCI_SAPU {
            let jendela = self.jendela;
            peta.retain(|_, q| {
                q.back()
                    .is_some_and(|t| sekarang.duration_since(*t) < jendela)
            });
        }
        let antrean = peta.entry(kunci.to_string()).or_default();
        while antrean
            .front()
            .is_some_and(|t| sekarang.duration_since(*t) >= self.jendela)
        {
            antrean.pop_front();
        }
        if antrean.len() as u32 >= self.batas {
            let tertua = antrean.front().copied().unwrap_or(sekarang);
            let sisa = self.jendela.saturating_sub(sekarang.duration_since(tertua));
            return Err(sisa.as_secs().max(1) + u64::from(sisa.subsec_nanos() > 0));
        }
        antrean.push_back(sekarang);
        Ok(())
    }
}

fn ringkas(awalan: &str, nilai: &str) -> String {
    let h = Sha256::digest(nilai.as_bytes());
    format!("{awalan}:{}", hex::encode(&h[..12]))
}

fn kunci_permintaan(req: &Request) -> String {
    let h = req.headers();
    if let Some(a) = h.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()) {
        return ringkas("tok", a);
    }
    if let Some(p) = h.get("x-zona-pin").and_then(|v| v.to_str().ok()) {
        return ringkas("pin", p);
    }
    match req.extensions().get::<ConnectInfo<SocketAddr>>() {
        Some(ConnectInfo(a)) => format!("ip:{}", a.ip()),
        None => "ip:anon".into(),
    }
}

/// Middleware: pasang dengan `axum::middleware::from_fn_with_state`.
pub async fn batasi(State(state): State<AppState>, req: Request, next: Next) -> Response {
    match state.pembatas.periksa(&kunci_permintaan(&req)) {
        Ok(()) => next.run(req).await,
        Err(retry_after) => AppError::KuotaHabis { retry_after }.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blokir_setelah_batas_dan_pulih_setelah_jendela() {
        let p = Pembatas::baru(2);
        let t0 = Instant::now();
        assert!(p.periksa_pada("a", t0).is_ok());
        assert!(p.periksa_pada("a", t0).is_ok());
        let retry = p
            .periksa_pada("a", t0 + Duration::from_secs(10))
            .unwrap_err();
        assert_eq!(retry, 50);
        assert!(
            p.periksa_pada("b", t0).is_ok(),
            "kunci lain tidak terpengaruh"
        );
        assert!(p.periksa_pada("a", t0 + Duration::from_secs(61)).is_ok());
    }
}
