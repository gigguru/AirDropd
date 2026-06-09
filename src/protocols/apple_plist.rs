//! Binary plist helpers for Apple AirDrop HTTPS endpoints.

use anyhow::{Context, Result};
use plist::{Dictionary, Value};

/// OpenDrop receiver flags: SUPPORTS_MIXED_TYPES | SUPPORTS_DISCOVER_MAYBE.
pub const RECEIVER_FLAGS_DISCOVERABLE: u32 = 0x08 | 0x80;

/// Reduced visibility (no /Discover support advertised).
pub const RECEIVER_FLAGS_HIDDEN: u32 = 0x02;

pub fn receiver_flags(discoverable: bool) -> u32 {
    if discoverable {
        RECEIVER_FLAGS_DISCOVERABLE
    } else {
        RECEIVER_FLAGS_HIDDEN
    }
}

/// Minimal media capabilities JSON embedded as bytes in the discover plist.
fn media_capabilities_json() -> Vec<u8> {
    br#"{"Version":1}"#.to_vec()
}

pub fn build_discover_response(computer_name: &str, model: &str) -> Result<Vec<u8>> {
    let mut dict = Dictionary::new();
    dict.insert(
        "ReceiverMediaCapabilities".to_string(),
        Value::Data(media_capabilities_json()),
    );
    dict.insert(
        "ReceiverComputerName".to_string(),
        Value::String(computer_name.to_string()),
    );
    dict.insert(
        "ReceiverModelName".to_string(),
        Value::String(model.to_string()),
    );
    encode_binary_plist(&dict)
}

pub fn build_ask_response(computer_name: &str, model: &str) -> Result<Vec<u8>> {
    let mut dict = Dictionary::new();
    dict.insert(
        "ReceiverModelName".to_string(),
        Value::String(model.to_string()),
    );
    dict.insert(
        "ReceiverComputerName".to_string(),
        Value::String(computer_name.to_string()),
    );
    encode_binary_plist(&dict)
}

pub fn parse_plist(body: &[u8]) -> Result<Value> {
    plist::from_bytes(body).context("parse binary plist request")
}

pub fn plist_string(value: &Value, key: &str) -> Option<String> {
    value.as_dictionary()?.get(key)?.as_string().map(str::to_string)
}

fn encode_binary_plist(dict: &Dictionary) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    Value::Dictionary(dict.clone())
        .to_writer_binary(&mut buf)
        .context("encode binary plist")?;
    Ok(buf)
}
