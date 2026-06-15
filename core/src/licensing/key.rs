//! `XXXX-XXXX-XXXX` product keys — offline HMAC validation.

use hmac::{Hmac, Mac};
use sha2::Sha256;
use thiserror::Error;

type HmacSha256 = Hmac<Sha256>;

/// Readable alphabet (32 chars): digits 2–9 plus A–Z without `I` or `L`.
pub const KEY_ALPHABET: &[u8; 32] = b"23456789ABCDEFGHJKMNOPQRSTUVWXYZ";

const KEY_LEN: usize = 12;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProductKeyError {
    #[error("invalid product key format")]
    InvalidFormat,
    #[error("invalid product key")]
    InvalidKey,
}

/// Strip hyphens and uppercase.
pub fn normalize_product_key(input: &str) -> String {
    input
        .chars()
        .filter(|c| *c != '-')
        .flat_map(|c| c.to_uppercase())
        .collect()
}

/// Format 12 raw characters as `XXXX-XXXX-XXXX`.
pub fn format_product_key(raw: &str) -> String {
    let n = normalize_product_key(raw);
    if n.len() != KEY_LEN {
        return raw.to_string();
    }
    format!(
        "{}-{}-{}",
        &n[0..4],
        &n[4..8],
        &n[8..12]
    )
}

fn decode_char(c: char) -> Option<u8> {
    KEY_ALPHABET.iter().position(|&b| b as char == c).map(|i| i as u8)
}

fn encode_value(mut value: u64, out: &mut [u8; KEY_LEN]) {
    for slot in out.iter_mut().rev() {
        *slot = KEY_ALPHABET[(value & 0x1F) as usize];
        value >>= 5;
    }
}

fn decode_value(raw: &[u8; KEY_LEN]) -> Option<u64> {
    let mut value = 0u64;
    for &b in raw {
        let idx = KEY_ALPHABET.iter().position(|&x| x == b)? as u64;
        value = (value << 5) | idx;
    }
    Some(value)
}

fn mac20(secret: &[u8], serial: u32) -> u32 {
    let mut mac =
        HmacSha256::new_from_slice(secret).expect("HMAC accepts any key length");
    mac.update(&serial.to_be_bytes());
    let digest = mac.finalize().into_bytes();
    let bytes = [digest[0], digest[1], digest[2], digest[3]];
    (u32::from_be_bytes(bytes) >> 12) & 0x000F_FFFF
}

/// Generate a product key (used by `airdropd-keygen`).
pub fn generate_product_key(secret: &[u8], serial: u32) -> String {
    let check = mac20(secret, serial);
    let combined = ((serial as u64) << 20) | (check as u64);
    let mut raw = [0u8; KEY_LEN];
    encode_value(combined, &mut raw);
    let s = std::str::from_utf8(&raw).expect("alphabet is ascii");
    format_product_key(s)
}

pub fn validate_product_key(key: &str, secret: &[u8]) -> Result<(), ProductKeyError> {
    let normalized = normalize_product_key(key);
    if normalized.len() != KEY_LEN || !normalized.chars().all(|c| decode_char(c).is_some()) {
        return Err(ProductKeyError::InvalidFormat);
    }
    let bytes: [u8; KEY_LEN] = normalized.as_bytes().try_into().unwrap();
    let combined = decode_value(&bytes).ok_or(ProductKeyError::InvalidFormat)?;
    let serial = (combined >> 20) as u32;
    let check = (combined & 0x000F_FFFF) as u32;
    if mac20(secret, serial) != check {
        return Err(ProductKeyError::InvalidKey);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_validate_generated_key() {
        let secret = b"test-secret";
        let key = generate_product_key(secret, 42_001);
        assert_eq!(normalize_product_key(&key).len(), 12);
        validate_product_key(&key, secret).unwrap();
    }

    #[test]
    fn rejects_tampered_key() {
        let secret = b"test-secret";
        let key = generate_product_key(secret, 99);
        let mut n = normalize_product_key(&key);
        if n.pop() == Some('Z') {
            n.push('2');
        }
        assert!(validate_product_key(&n, secret).is_err());
    }
}
