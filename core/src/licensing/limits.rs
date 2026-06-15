//! Demo quotas and registered entitlements.

pub const DEMO_SENDS_PER_WEEK: u32 = 3;
pub const DEMO_QR_UPLOADS_PER_WEEK: u32 = 3;
pub const DEMO_MAX_FILE_BYTES: u64 = 15 * 1024 * 1024;
pub const MAX_ACTIVATIONS_PER_KEY: usize = 2;

pub fn demo_sends_per_week() -> u32 {
    DEMO_SENDS_PER_WEEK
}

pub fn demo_qr_uploads_per_week() -> u32 {
    DEMO_QR_UPLOADS_PER_WEEK
}

pub fn demo_max_file_bytes() -> u64 {
    DEMO_MAX_FILE_BYTES
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LicenseStatus {
    Demo,
    Registered,
}
