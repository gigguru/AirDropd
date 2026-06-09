use std::collections::HashMap;
use sha2::{Digest, Sha256};
use anyhow::Result;

use super::apple_plist;

/// Apple-specific TXT record generator for AirDrop mDNS services.
pub struct AppleRecords;

impl AppleRecords {
    /// Stable 6-byte hex hash used in Apple `ph` TXT key and BLE manufacturer data.
    pub fn stable_device_ph(seed: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(seed.as_bytes());
        hasher.update(b"AirDropd-ph-v1");
        hex::encode(&hasher.finalize()[..6])
    }

    /// OpenDrop-compatible minimal TXT: only `flags` (required for /Discover probing).
    pub fn create_airdrop_txt_records(discoverable: bool) -> Result<HashMap<String, String>> {
        let mut properties = HashMap::new();
        properties.insert(
            "flags".to_string(),
            apple_plist::receiver_flags(discoverable).to_string(),
        );
        Ok(properties)
    }

    /// Extended TXT records used by some Apple devices (optional enrichment).
    pub fn create_airdrop_txt_records_with_name(
        display_name: &str,
        device_ph: &str,
        discoverable: bool,
    ) -> Result<HashMap<String, String>> {
        let mut properties = Self::create_airdrop_txt_records(discoverable)?;
        properties.insert("name".to_string(), display_name.to_string());
        properties.insert("model".to_string(), "Windows,1".to_string());
        properties.insert("ph".to_string(), device_ph.to_string());
        properties.insert("pv".to_string(), "2".to_string());
        Ok(properties)
    }

    #[allow(dead_code)]
    pub fn create_companion_txt_records_with_name(
        display_name: &str,
        device_id: &str,
        device_ph: &str,
    ) -> Result<HashMap<String, String>> {
        let mut properties = HashMap::new();
        properties.insert("rpMRtID".to_string(), device_id.to_string());
        properties.insert("rpAD".to_string(), device_ph.to_string());
        properties.insert("rpVr".to_string(), "350.92.4".to_string());
        properties.insert("rpFl".to_string(), "0x20000".to_string());
        properties.insert("rpHA".to_string(), device_ph.to_string());
        properties.insert("rpHI".to_string(), device_id.to_string());
        properties.insert("rpMd".to_string(), "Windows,1".to_string());
        properties.insert("rpNm".to_string(), display_name.to_string());
        Ok(properties)
    }

    #[allow(dead_code)]
    pub fn create_device_info_txt_records(device_ph: &str) -> Result<HashMap<String, String>> {
        let mut properties = HashMap::new();
        properties.insert("model".to_string(), "Windows,1".to_string());
        properties.insert("osxvers".to_string(), "10".to_string());
        properties.insert("srcvers".to_string(), "350.92.4".to_string());
        properties.insert("features".to_string(), "0x445F8A00,0x1C340".to_string());
        properties.insert("flags".to_string(), "0x4".to_string());
        properties.insert("vv".to_string(), "2".to_string());
        properties.insert("pk".to_string(), device_ph.to_string());
        Ok(properties)
    }
}
