//! AWDL protocol constants (from OWL / Seemoo research).

/// Fixed AWDL BSSID used in all AWDL frames.
pub const AWDL_BSSID: [u8; 6] = [0x00, 0x25, 0x00, 0xff, 0x94, 0x73];

/// Apple vendor OUI used in LLC/SNAP headers.
pub const APPLE_OUI: [u8; 3] = [0x00, 0x17, 0xf2];

/// AWDL vendor-specific action frame OUI type.
pub const AWDL_OUI_TYPE: u8 = 0x08;

/// AWDL protocol version embedded in action frames.
pub const AWDL_VERSION: u16 = 0x0001;

/// Social channels used for AWDL channel hopping (2.4 / 5 GHz).
pub const SOCIAL_CHANNELS: [(u8, u8); 3] = [
    (6, 0x51),    // channel 6
    (44, 0x80),   // channel 44
    (149, 0x80),  // channel 149
];

/// Channel sequence length.
pub const CHANSEQ_LENGTH: usize = 16;

/// Infrastructure-mode AWDL sync multicast (UDP fallback when raw 802.11 unavailable).
pub const INFRA_MULTICAST: &str = "224.0.0.253";
pub const INFRA_PORT: u16 = 5356;

/// PSF transmission interval (matches OWL ~110 ms).
pub const PSF_INTERVAL_MS: u64 = 110;

/// Peer timeout.
pub const PEER_TIMEOUT_SECS: i64 = 30;
