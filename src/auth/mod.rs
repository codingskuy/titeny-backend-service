//! Autentikasi: JWT `aud=titeny`, refresh berkeluarga, OTP analis/CEO, PIN zona.

pub mod extract;
pub mod handlers;
pub mod jwt;
pub mod otp;
pub mod password;
pub mod pin;
pub mod service;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Peran {
    Ceo,
    Analis,
    Layanan,
}

impl Peran {
    pub fn dari_str(s: &str) -> Option<Self> {
        match s {
            "ceo" => Some(Self::Ceo),
            "analis" => Some(Self::Analis),
            "layanan" => Some(Self::Layanan),
            _ => None,
        }
    }
    pub fn sebagai_str(self) -> &'static str {
        match self {
            Self::Ceo => "ceo",
            Self::Analis => "analis",
            Self::Layanan => "layanan",
        }
    }
}
