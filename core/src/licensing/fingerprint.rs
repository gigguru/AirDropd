//! Stable, cross-platform machine fingerprint for license activation.

use sha2::{Digest, Sha256};

/// Short hex fingerprint used for activation records (same algorithm on Windows and macOS).
pub fn machine_fingerprint() -> String {
    let seed = fingerprint_seed();
    let mut hasher = Sha256::new();
    hasher.update(seed.as_bytes());
    hasher.update(b"AirDropd-machine-v1");
    hex::encode(&hasher.finalize()[..8])
}

fn fingerprint_seed() -> String {
    let host = hostname::get()
        .ok()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|| "AirDropd".to_string());
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    format!("{host}|{os}|{arch}")
}
