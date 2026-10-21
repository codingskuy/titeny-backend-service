//! Hash kata sandi argon2id dan helper digest token.

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::RngCore;
use sha2::{Digest, Sha256};

use crate::error::{AppError, Hasil};

pub fn hash_password(password: &str) -> Hasil<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(AppError::server)
}

pub fn cocok(password: &str, hash: &str) -> bool {
    PasswordHash::new(hash)
        .map(|h| {
            Argon2::default()
                .verify_password(password.as_bytes(), &h)
                .is_ok()
        })
        .unwrap_or(false)
}

/// Token opak acak 256 bit (base64url) untuk refresh token.
pub fn token_acak() -> String {
    let mut b = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut b);
    URL_SAFE_NO_PAD.encode(b)
}

pub fn digest(pepper: &str, nilai: &str) -> String {
    let mut h = Sha256::new();
    h.update(pepper.as_bytes());
    h.update(b"\x00");
    h.update(nilai.as_bytes());
    hex::encode(h.finalize())
}
