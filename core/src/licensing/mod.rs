//! Demo vs registered licensing for AirDropd.
//!
//! Demo: 3 AirDrop sends / week, 3 QR uploads / week (Web Drop + DJ Mode), 15 MB max.
//! Registered: unlimited — unlocked with a valid `XXXX-XXXX-XXXX` product key.

pub mod key;
mod fingerprint;
mod limits;
mod store;

pub use fingerprint::machine_fingerprint;
pub use key::{format_product_key, normalize_product_key, validate_product_key, ProductKeyError};
pub use limits::{
    demo_max_file_bytes, demo_qr_uploads_per_week, demo_sends_per_week, LicenseStatus,
    MAX_ACTIVATIONS_PER_KEY,
};
pub use store::{
    ActivationError, DemoLimitError, GatedFeature, LicenseFields, LicenseStore,
};

/// Compile-time HMAC secret for product keys (must match `airdropd-keygen`).
const KEY_SECRET: &[u8] = b"AirDropd-RhythmicRecords-2026-v1";

pub fn validate_key(key: &str) -> Result<(), ProductKeyError> {
    validate_product_key(key, KEY_SECRET)
}

pub fn is_registered(store: &LicenseStore) -> bool {
    store.status() == LicenseStatus::Registered
}
