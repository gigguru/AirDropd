use mdns_sd::{ServiceEvent, ServiceInfo};
use std::sync::Arc;
use tokio::sync::Mutex;
use anyhow::Result;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr, Ipv4Addr};
use tracing::{info, error, warn};
use std::sync::atomic::{AtomicBool, Ordering};
use socket2::{Socket, Domain, Type, Protocol};
use uuid::Uuid;
use super::interface::NetworkManager;
use super::mdns_hub::SharedMdns;
use super::util::friendly_name;

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct DiscoveredDevice {
	pub name: String,
	pub address: IpAddr,
	#[allow(dead_code)]
	pub port: u16,
	pub service_type: ServiceType,
	pub txt_records: HashMap<String, String>,
	/// BLE signal strength in dBm when the device is also seen over Bluetooth.
	/// Used to approximate physical distance on the radar.
	pub rssi: Option<i16>,
}

/// Physical device category, derived from mDNS TXT records and the name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceKind {
	IPhone,
	IPad,
	IPod,
	MacBook,
	IMac,
	/// Mac mini, Mac Studio, Mac Pro, and other desktop Macs.
	MacDesktop,
	AppleWatch,
	AppleTv,
	AirPods,
	AirTag,
	/// A lost Apple device broadcasting in Find My mode (not an AirTag).
	FindMyDevice,
	WindowsPc,
	AndroidPhone,
	AndroidTablet,
	AndroidWatch,
	AndroidTag,
	Unknown,
}

impl DeviceKind {
	/// Short human-readable label, e.g. "iPhone" or "MacBook".
	pub fn label(&self) -> &'static str {
		match self {
			DeviceKind::IPhone => "iPhone",
			DeviceKind::IPad => "iPad",
			DeviceKind::IPod => "iPod",
			DeviceKind::MacBook => "MacBook",
			DeviceKind::IMac => "iMac",
			DeviceKind::MacDesktop => "Mac",
			DeviceKind::AppleWatch => "Apple Watch",
			DeviceKind::AppleTv => "Apple TV",
			DeviceKind::AirPods => "AirPods",
			DeviceKind::AirTag => "AirTag",
			DeviceKind::FindMyDevice => "Find My device",
			DeviceKind::WindowsPc => "Windows PC",
			DeviceKind::AndroidPhone => "Android phone",
			DeviceKind::AndroidTablet => "Android tablet",
			DeviceKind::AndroidWatch => "Android watch",
			DeviceKind::AndroidTag => "Android tag",
			DeviceKind::Unknown => "Nearby device",
		}
	}

	/// Icon used on the radar and in device lists.
	pub fn emoji(&self) -> &'static str {
		match self {
			DeviceKind::IPhone => "📱",
			DeviceKind::IPad => "📲",
			DeviceKind::IPod => "🎵",
			DeviceKind::MacBook => "💻",
			DeviceKind::IMac => "🖥",
			DeviceKind::MacDesktop => "🖳",
			DeviceKind::AppleWatch => "⌚",
			DeviceKind::AppleTv => "📺",
			DeviceKind::AirPods => "🎧",
			DeviceKind::AirTag => "🏷",
			DeviceKind::FindMyDevice => "📍",
			DeviceKind::WindowsPc => "🪟",
			DeviceKind::AndroidPhone => "🤖",
			DeviceKind::AndroidTablet => "📟",
			DeviceKind::AndroidWatch => "⌚",
			DeviceKind::AndroidTag => "🏷",
			DeviceKind::Unknown => "📡",
		}
	}
}

/// Map an Apple hardware identifier ("iPhone14,2", "MacBookPro18,1", "J274AP")
/// to a device category.
fn kind_from_model(model: &str) -> Option<DeviceKind> {
	let m = model.trim();
	if m.is_empty() {
		return None;
	}
	let ml = m.to_ascii_lowercase();
	if ml.starts_with("windows") {
		return Some(DeviceKind::WindowsPc);
	}
	if ml.starts_with("iphone") {
		return Some(DeviceKind::IPhone);
	}
	if ml.starts_with("ipad") {
		return Some(DeviceKind::IPad);
	}
	if ml.starts_with("ipod") {
		return Some(DeviceKind::IPod);
	}
	if ml.starts_with("watch") {
		return Some(DeviceKind::AppleWatch);
	}
	if ml.starts_with("appletv") || ml.starts_with("audioaccessory") {
		return Some(DeviceKind::AppleTv);
	}
	if ml.starts_with("macbook") {
		return Some(DeviceKind::MacBook);
	}
	if ml.starts_with("imac") {
		return Some(DeviceKind::IMac);
	}
	if ml.starts_with("macmini")
		|| ml.starts_with("macstudio")
		|| ml.starts_with("macpro")
		|| ml.starts_with("mac1")
		|| ml.starts_with("virtualmac")
	{
		return Some(DeviceKind::MacDesktop);
	}
	if ml.starts_with("mac") {
		return Some(DeviceKind::MacDesktop);
	}
	None
}

/// BLE manufacturer IDs commonly used by Android phone OEMs.
const ANDROID_BLE_MANUFACTURERS: &[u16] = &[
	0x00E0, // Google
	0x0075, // Samsung
	0x0159, // Xiaomi
	0x038F, // Xiaomi (alternate)
	0x0104, // OnePlus / BBK
	0x0171, // Google Fast Pair
	0x0060, // Motorola / Lenovo
	0x027D, // Huawei
	0x039E, // Oppo
	0x0310, // Vivo
	0x022B, // Honor
	0x05A7, // Nothing
	0x0200, // Transsion (Tecno / Infinix)
	0x0183, // Sony Mobile
	0x008A, // BBK / Realme ecosystem
];

/// Google Fast Pair — common on Android phones even when the local name is hidden.
const FAST_PAIR_SERVICE: Uuid = Uuid::from_u128(0x0000fe2c_0000_1000_8000_00805f9b34fb);

/// Nearby Share / Samsung Quick Share often use this service slot on Android.
const NEARBY_SERVICE: Uuid = Uuid::from_u128(0x0000fe2c_0000_1000_8000_00805f9b34fb);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BleMobilePlatform {
	Apple,
	Android,
}

/// BLE advertisement hints used to surface phones on the sonar without a name or model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BleMobileProfile {
	pub is_mobile: bool,
	pub platform: Option<BleMobilePlatform>,
	/// `phone`, `tablet`, `watch`, or `tag`.
	pub device_class: &'static str,
}

impl BleMobileProfile {
	pub fn is_phone_like(self) -> bool {
		self.is_mobile && !matches!(self.device_class, "tag")
	}
}

fn is_android_platform(txt: &HashMap<String, String>) -> bool {
	txt.get("platform")
		.map(|p| p.eq_ignore_ascii_case("android"))
		.unwrap_or(false)
}

fn kind_from_android(name: &str, txt: &HashMap<String, String>) -> Option<DeviceKind> {
	if !is_android_platform(txt) {
		return None;
	}
	if let Some(class) = txt.get("device_class") {
		return match class.as_str() {
			"tablet" => Some(DeviceKind::AndroidTablet),
			"watch" => Some(DeviceKind::AndroidWatch),
			"tag" => Some(DeviceKind::AndroidTag),
			"phone" => Some(DeviceKind::AndroidPhone),
			_ => None,
		};
	}

	let nl = name.to_ascii_lowercase();
	if nl.contains("watch") || nl.contains("wear os") || nl.contains("galaxy watch") {
		return Some(DeviceKind::AndroidWatch);
	}
	if nl.contains("tablet")
		|| nl.contains(" tab")
		|| nl.contains("pad")
		|| nl.contains("sm-t")
		|| nl.contains("pixel tablet")
	{
		return Some(DeviceKind::AndroidTablet);
	}
	if nl.contains("tag")
		|| nl.contains("tile")
		|| nl.contains("smarttag")
		|| nl.contains("tracker")
	{
		return Some(DeviceKind::AndroidTag);
	}
	Some(DeviceKind::AndroidPhone)
}

fn kind_from_accessory(label: &str, name: &str) -> Option<DeviceKind> {
	let nl = name.to_ascii_lowercase();
	match label {
		"AirPods" => Some(DeviceKind::AirPods),
		"AirPlay device" => Some(DeviceKind::AppleTv),
		"Find My device" => {
			if nl.contains("airtag") {
				Some(DeviceKind::AirTag)
			} else if nl.contains("iphone")
				|| nl.contains("ipad")
				|| nl.contains("macbook")
				|| nl.contains("imac")
				|| nl.contains("watch")
			{
				Some(DeviceKind::FindMyDevice)
			} else {
				// Most 0x12-only beacons are AirTags.
				Some(DeviceKind::AirTag)
			}
		}
		"Beacon" => Some(DeviceKind::AirTag),
		_ => None,
	}
}

fn kind_from_name(name: &str) -> Option<DeviceKind> {
	let nl = name.to_ascii_lowercase();
	if nl.contains("iphone") {
		return Some(DeviceKind::IPhone);
	}
	if nl.contains("ipad") {
		return Some(DeviceKind::IPad);
	}
	if nl.contains("ipod") {
		return Some(DeviceKind::IPod);
	}
	if nl.contains("macbook") {
		return Some(DeviceKind::MacBook);
	}
	if nl.contains("imac") {
		return Some(DeviceKind::IMac);
	}
	if nl.contains("mac mini") || nl.contains("mac studio") || nl.contains("mac pro") {
		return Some(DeviceKind::MacDesktop);
	}
	if nl.contains("apple watch") || (nl.contains("watch") && !nl.contains("android")) {
		return Some(DeviceKind::AppleWatch);
	}
	if nl.contains("apple tv") {
		return Some(DeviceKind::AppleTv);
	}
	if nl.contains("airpods") {
		return Some(DeviceKind::AirPods);
	}
	if nl.contains("airtag") {
		return Some(DeviceKind::AirTag);
	}
	if nl.contains("find my") {
		return Some(DeviceKind::FindMyDevice);
	}
	if nl.contains("pixel")
		|| nl.contains("galaxy")
		|| nl.contains("android")
		|| nl.contains("oneplus")
		|| nl.contains("xiaomi")
	{
		return kind_from_android(name, &HashMap::from([(
			"platform".to_string(),
			"android".to_string(),
		)]));
	}
	if nl.contains("pc") || nl.contains("windows") {
		return Some(DeviceKind::WindowsPc);
	}
	None
}

/// Whether a device kind counts as an Apple file-transfer peer.
pub fn is_apple_transfer_device(kind: DeviceKind) -> bool {
    matches!(
        kind,
        DeviceKind::IPhone
            | DeviceKind::IPad
            | DeviceKind::IPod
            | DeviceKind::MacBook
            | DeviceKind::IMac
            | DeviceKind::MacDesktop
    )
}

pub fn is_accessory_kind(kind: DeviceKind) -> bool {
    matches!(
        kind,
        DeviceKind::AirPods | DeviceKind::AirTag | DeviceKind::FindMyDevice
    )
}

/// Primary sonar targets: phones, tablets, and computers that can share files.
pub fn is_sonar_peer_kind(kind: DeviceKind) -> bool {
    matches!(
        kind,
        DeviceKind::IPhone
            | DeviceKind::IPad
            | DeviceKind::IPod
            | DeviceKind::MacBook
            | DeviceKind::IMac
            | DeviceKind::MacDesktop
            | DeviceKind::AndroidPhone
            | DeviceKind::AndroidTablet
            | DeviceKind::AndroidWatch
            | DeviceKind::WindowsPc
    )
}

pub fn is_android_device(kind: DeviceKind) -> bool {
    matches!(
        kind,
        DeviceKind::AndroidPhone
            | DeviceKind::AndroidTablet
            | DeviceKind::AndroidWatch
            | DeviceKind::AndroidTag
    )
}

pub fn is_airtag_beacon(kind: DeviceKind) -> bool {
    matches!(kind, DeviceKind::AirTag | DeviceKind::FindMyDevice)
}

impl DiscoveredDevice {
	/// Whether this device should appear on the sonar for the given discovery mode.
	pub fn is_sonar_visible(&self, show_all: bool) -> bool {
		if show_all {
			return true;
		}
		let kind = self.kind();
		if is_accessory_kind(kind) || kind == DeviceKind::AppleTv {
			return false;
		}
		if is_sonar_peer_kind(kind) {
			return true;
		}
		if self.txt_records.get("mobile_presence") == Some(&"1".to_string()) {
			return true;
		}
		if self.is_reachable()
			&& matches!(
				self.service_type,
				ServiceType::AirDrop | ServiceType::Companion | ServiceType::DeviceInfo
			)
		{
			return true;
		}
		false
	}

	fn has_apple_mobile_hint(&self) -> bool {
		self.txt_records.get("apple_presence") == Some(&"1".to_string())
			|| (self.txt_records.get("mobile_presence") == Some(&"1".to_string())
				&& !is_android_platform(&self.txt_records))
	}

	/// Apply the radar filter from the main-view discovery picker.
	pub fn matches_filter(&self, filter: crate::config::DeviceFilter, show_all: bool) -> bool {
		let kind = self.kind();
		match filter {
			crate::config::DeviceFilter::All => self.is_sonar_visible(show_all),
			crate::config::DeviceFilter::Apple => {
				is_apple_transfer_device(kind) || (kind == DeviceKind::Unknown && self.has_apple_mobile_hint())
			}
			crate::config::DeviceFilter::Android => is_android_device(kind),
			crate::config::DeviceFilter::AirTags => is_airtag_beacon(kind),
		}
	}

	/// Best-effort device category from TXT records, falling back to the name.
	pub fn kind(&self) -> DeviceKind {
		if let Some(kind) = kind_from_android(&self.name, &self.txt_records) {
			return kind;
		}

		if let Some(label) = self.txt_records.get("accessory_label") {
			if let Some(kind) = kind_from_accessory(label, &self.name) {
				return kind;
			}
		}

		// Hardware identifiers from _device-info (model), companion-link
		// (rpMd), and AirPlay (model) advertisements.
		for key in ["model", "rpMd", "am", "usb_mdl", "rmodel", "modelName", "md"] {
			if let Some(kind) = self
				.txt_records
				.get(key)
				.and_then(|model| kind_from_model(model))
			{
				return kind;
			}
		}

		if let Some(kind) = kind_from_os_version(&self.txt_records) {
			return kind;
		}

		if let Some(kind) = kind_from_name(&self.name) {
			return kind;
		}

		// Anonymous Apple Continuity beacons (phones/tablets with no BLE name).
		if self.txt_records.get("anonymous") == Some(&"1".to_string()) {
			if self.txt_records.get("apple_presence") == Some(&"1".to_string()) {
				return kind_from_mobile_presence(&self.txt_records);
			}
			if self.txt_records.get("airdrop_active") == Some(&"1".to_string()) {
				return kind_from_mobile_presence(&self.txt_records);
			}
		}

		if self.txt_records.get("mobile_presence") == Some(&"1".to_string()) {
			return kind_from_mobile_presence(&self.txt_records);
		}

		if matches!(self.service_type, ServiceType::IosMobile) {
			return DeviceKind::IPhone;
		}

		DeviceKind::Unknown
	}

	/// Whether this device should appear on the sonar as a phone/tablet-class peer.
	pub fn is_mobile_device(&self) -> bool {
		matches!(
			self.kind(),
			DeviceKind::IPhone
				| DeviceKind::IPad
				| DeviceKind::IPod
				| DeviceKind::AndroidPhone
				| DeviceKind::AndroidTablet
				| DeviceKind::AndroidWatch
		) || self.txt_records.get("mobile_presence") == Some(&"1".to_string())
	}

	/// Hardware model identifier from mDNS TXT records (e.g. `iPhone14,2`).
	pub fn hardware_identifier(&self) -> Option<String> {
		for key in ["model", "rpMd", "am", "usb_mdl"] {
			if let Some(v) = self.txt_records.get(key) {
				let t = v.trim();
				if !t.is_empty() {
					return Some(t.to_string());
				}
			}
		}
		None
	}

	/// User-visible title: discovered name when meaningful, otherwise hardware id.
	pub fn display_title(&self) -> String {
		if !is_generic_device_name(&self.name) {
			return self.name.clone();
		}
		if let Some(hw) = self.hardware_identifier() {
			return hw;
		}
		anonymous_display_title(&self.name, self.kind())
	}

	/// Whether the device can receive Wi‑Fi transfers (not BLE-only).
	pub fn is_reachable(&self) -> bool {
		!self.address.is_unspecified() && self.port > 0
	}

	/// Short status for list views.
	pub fn status_label(&self) -> &'static str {
		if self.is_reachable() {
			"Ready"
		} else {
			"Bluetooth only"
		}
	}

	/// Stable key for matching the same device across discovery refreshes.
	pub fn match_key(&self) -> String {
		if let Some(id) = self.txt_records.get("ble_id") {
			return format!("ble:{id}");
		}
		format!("{}|{}", self.address, self.name)
	}

	/// Human-readable discovery service label.
	pub fn service_label(&self) -> &'static str {
		match self.service_type {
			ServiceType::AirDrop => "AirDrop",
			ServiceType::AirPlay => "AirPlay",
			ServiceType::Raop => "AirPlay audio",
			ServiceType::Companion => "Companion link",
			ServiceType::DeviceInfo => "Device info",
			_ => "Nearby service",
		}
	}

	/// Whether the device currently has an active AirDrop share sheet (TXT hint).
	pub fn airdrop_active(&self) -> bool {
		self.txt_records.get("airdrop_active").map(|v| v == "1").unwrap_or(false)
	}
}

fn is_generic_device_name(name: &str) -> bool {
	let n = name.trim();
	if n.is_empty() {
		return true;
	}
	let nl = n.to_ascii_lowercase();
	nl.starts_with("apple device ")
		|| nl.starts_with("iphone nearby")
		|| nl.starts_with("ipad nearby")
		|| nl.starts_with("android phone")
		|| nl.starts_with("android tablet")
		|| nl.starts_with("android watch")
		|| nl.starts_with("mobile phone")
		|| nl == "unknown"
		|| nl == "unknown device"
		|| nl.starts_with("nearby ")
}

fn kind_from_mobile_presence(txt: &HashMap<String, String>) -> DeviceKind {
	let class = txt.get("device_class").map(|s| s.as_str()).unwrap_or("phone");
	if is_android_platform(txt) {
		return match class {
			"tablet" => DeviceKind::AndroidTablet,
			"watch" => DeviceKind::AndroidWatch,
			"tag" => DeviceKind::AndroidTag,
			_ => DeviceKind::AndroidPhone,
		};
	}
	if txt.get("apple_presence").map(|v| v == "1").unwrap_or(false)
		|| txt.get("anonymous").map(|v| v == "1").unwrap_or(false)
	{
		return match class {
			"tablet" => DeviceKind::IPad,
			"watch" => DeviceKind::AppleWatch,
			_ => DeviceKind::IPhone,
		};
	}
	DeviceKind::Unknown
}

/// Infer Mac vs iPhone/iPad from mDNS OS version strings when model is missing.
fn kind_from_os_version(txt: &HashMap<String, String>) -> Option<DeviceKind> {
	for key in ["osvers", "OSVersion", "systemVersion"] {
		let Some(raw) = txt.get(key) else {
			continue;
		};
		let v = raw.trim().to_ascii_lowercase();
		if v.contains("macos") || v.contains("mac os") {
			return Some(DeviceKind::MacDesktop);
		}
		if v.contains("iphone") {
			return Some(DeviceKind::IPhone);
		}
		if v.contains("ipad") {
			return Some(DeviceKind::IPad);
		}
		if v.chars().next()?.is_ascii_digit() {
			// Bare iOS-style semver (e.g. "17.2") on AirPlay / companion ads.
			return Some(DeviceKind::IPhone);
		}
	}
	None
}

fn anonymous_display_title(name: &str, kind: DeviceKind) -> String {
	let suffix = name
		.split_whitespace()
		.last()
		.filter(|s| !s.is_empty())
		.unwrap_or("nearby");
	match kind {
		DeviceKind::IPhone => format!("iPhone nearby ({suffix})"),
		DeviceKind::IPad => format!("iPad nearby ({suffix})"),
		DeviceKind::AndroidPhone => format!("Android phone ({suffix})"),
		DeviceKind::AndroidTablet => format!("Android tablet ({suffix})"),
		DeviceKind::AndroidWatch => format!("Android watch ({suffix})"),
		DeviceKind::Unknown => format!("Mobile phone ({suffix})"),
		_ => name.to_string(),
	}
}

fn android_name_hints(name: &str) -> bool {
	let nl = name.to_ascii_lowercase();
	nl.contains("pixel")
		|| nl.contains("galaxy")
		|| nl.contains("android")
		|| nl.contains("oneplus")
		|| nl.contains("xiaomi")
		|| nl.contains("sm-")
		|| nl.contains("motorola")
		|| nl.contains("oppo")
		|| nl.contains("vivo")
		|| nl.contains("realme")
		|| nl.contains("nothing phone")
		|| nl.contains("huawei")
		|| nl.contains("honor")
}

fn classify_android_device_class(name: &str) -> &'static str {
	let nl = name.to_ascii_lowercase();
	if nl.contains("watch") || nl.contains("wear os") || nl.contains("galaxy watch") {
		"watch"
	} else if nl.contains("tablet")
		|| nl.contains(" tab")
		|| nl.contains("pad")
		|| nl.contains("sm-t")
		|| nl.contains("pixel tablet")
	{
		"tablet"
	} else if nl.contains("tag")
		|| nl.contains("tile")
		|| nl.contains("smarttag")
		|| nl.contains("tracker")
	{
		"tag"
	} else {
		"phone"
	}
}

fn has_android_manufacturer(manufacturer_ids: impl IntoIterator<Item = u16>) -> bool {
	manufacturer_ids
		.into_iter()
		.any(|id| ANDROID_BLE_MANUFACTURERS.contains(&id))
}

fn advertises_fast_pair(
	service_data: &HashMap<Uuid, Vec<u8>>,
	services: &[Uuid],
) -> bool {
	service_data.contains_key(&FAST_PAIR_SERVICE)
		|| services.iter().any(|uuid| *uuid == FAST_PAIR_SERVICE || *uuid == NEARBY_SERVICE)
}

fn ble_class_is_phone(class: u32) -> bool {
	// Major device class (bits 8-12): 0x02 = phone.
	((class >> 8) & 0x1F) == 0x02
}

fn android_payload_heuristic(manufacturer_data: &HashMap<u16, Vec<u8>>) -> bool {
	for (id, payload) in manufacturer_data {
		if *id == 0x0075 && payload.len() >= 2 {
			// Samsung manufacturer-specific payloads on phones/tablets.
			return true;
		}
		if (*id == 0x00E0 || *id == 0x0171) && !payload.is_empty() {
			// Google / Fast Pair manufacturer blocks.
			return true;
		}
	}
	false
}

/// Classify an Android device from BLE manufacturer data and its local name.
pub fn android_class_from_ble(
	name: &str,
	manufacturer_ids: impl IntoIterator<Item = u16>,
) -> Option<&'static str> {
	if !has_android_manufacturer(manufacturer_ids) && !android_name_hints(name) {
		return None;
	}
	Some(classify_android_device_class(name))
}

/// Best-effort mobile classification from a raw BLE advertisement.
#[allow(clippy::too_many_arguments)]
pub fn ble_mobile_profile(
	name: &str,
	manufacturer_data: &HashMap<u16, Vec<u8>>,
	service_data: &HashMap<Uuid, Vec<u8>>,
	services: &[Uuid],
	class: Option<u32>,
	apple_presence: bool,
	apple_airdrop: bool,
	apple_accessory_only: bool,
) -> BleMobileProfile {
	if apple_accessory_only {
		return BleMobileProfile::default();
	}

	if apple_presence || apple_airdrop {
		return BleMobileProfile {
			is_mobile: true,
			platform: Some(BleMobilePlatform::Apple),
			device_class: classify_apple_mobile_class(name),
		};
	}

	if let Some(device_class) = android_class_from_ble(name, manufacturer_data.keys().copied()) {
		return BleMobileProfile {
			is_mobile: device_class != "tag",
			platform: Some(BleMobilePlatform::Android),
			device_class,
		};
	}

	if advertises_fast_pair(service_data, services)
		|| android_payload_heuristic(manufacturer_data)
	{
		return BleMobileProfile {
			is_mobile: true,
			platform: Some(BleMobilePlatform::Android),
			device_class: classify_android_device_class(name),
		};
	}

	if class.map(ble_class_is_phone).unwrap_or(false) {
		return BleMobileProfile {
			is_mobile: true,
			platform: None,
			device_class: "phone",
		};
	}

	if !name.is_empty() && android_name_hints(name) {
		return BleMobileProfile {
			is_mobile: true,
			platform: Some(BleMobilePlatform::Android),
			device_class: classify_android_device_class(name),
		};
	}

	BleMobileProfile::default()
}

fn classify_apple_mobile_class(name: &str) -> &'static str {
	let nl = name.to_ascii_lowercase();
	if nl.contains("ipad") {
		"tablet"
	} else if nl.contains("ipod") {
		"phone"
	} else {
		"phone"
	}
}

#[derive(Clone, Debug, PartialEq)]
pub enum ServiceType {
	AirPlay,
	AirDrop,
	Raop,
	Companion,
	DeviceInfo,
	#[allow(dead_code)]
	IosMobile,
	#[allow(dead_code)]
	IosPairable,
	#[allow(dead_code)]
	IosContinuity,
	#[allow(dead_code)]
	DnsService,
	#[allow(dead_code)]
	Homekit,
	#[allow(dead_code)]
	AirPrint,
	#[allow(dead_code)]
	AppleTV,
	#[allow(dead_code)]
	RemoteDevice,
	#[allow(dead_code)]
	HomeSharing,
	#[allow(dead_code)]
	AppleMidi,
	#[allow(dead_code)]
	AirPort,
	#[allow(dead_code)]
	AppleAuth,
	#[allow(dead_code)]
	Presence,
}

#[allow(dead_code)]
pub struct DeviceDiscovery {
	mdns: SharedMdns,
	devices: Arc<Mutex<HashMap<String, DiscoveredDevice>>>,
	running: Arc<AtomicBool>,
	network_manager: NetworkManager,
}

impl DeviceDiscovery {
	#[allow(dead_code)]
	fn check_network_availability() -> Result<()> {
		info!("Checking network availability for mDNS...");
		
		let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
		socket.set_reuse_address(true)?;
		
		let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0);
		if let Err(e) = socket.bind(&addr.into()) {
			error!("Network binding error: {}", e);
			return Err(e.into());
		}

		// Check multicast capability
		let multicast_addr: Ipv4Addr = "224.0.0.251".parse()?;
		let interfaces = local_ip_address::list_afinet_netifas()?;
		let mut has_valid_interface = false;

		for (name, ip) in interfaces {
			if let IpAddr::V4(interface_addr) = ip {
				if !ip.is_loopback() && !interface_addr.is_link_local() {
					has_valid_interface = true;
					if let Err(e) = socket.join_multicast_v4(&multicast_addr, &interface_addr) {
						warn!("Failed to join multicast on {}: {}", name, e);
					} else {
						info!("Successfully joined multicast group on interface {} ({})", name, interface_addr);
					}
				}
			}
		}

		if !has_valid_interface {
			error!("No valid network interfaces found for multicast");
			return Err(anyhow::anyhow!("No valid network interfaces"));
		}

		info!("Network is available with multicast support");
		Ok(())
	}

	pub fn new(mdns: SharedMdns) -> Result<Self> {
		info!("Initializing device discovery service");
		
		let mut network_manager = NetworkManager::new()?;
		network_manager.initialize()?;

		let multicast_addr: Ipv4Addr = "224.0.0.251".parse()?;
		network_manager.join_multicast_group(multicast_addr)?;

		Ok(Self {
			mdns,
			devices: Arc::new(Mutex::new(HashMap::new())),
			running: Arc::new(AtomicBool::new(false)),
			network_manager,
		})
	}

	pub async fn start_discovery(&self) -> Result<()> {
		if self.running.load(Ordering::SeqCst) {
			return Ok(());
		}
		self.running.store(true, Ordering::SeqCst);

		info!("Starting device discovery service...");
		
		let service_types = [
			"_airplay._tcp.local.",
			"_raop._tcp.local.",
			"_airdrop._tcp.local.",
			"_airdrop._udp.local.",
			"_companion-link._tcp.local.",
			"_device-info._tcp.local.",
			"_apple-mobdev2._tcp.local.",
			"_rdlink._tcp.local.",
		];

		for &service_type in &service_types {
			match self.mdns.browse(service_type) {
				Ok(receiver) => {
					let devices = self.devices.clone();
					let service_type = service_type.to_string();
					let running = self.running.clone();
					
					tokio::spawn(async move {
						while running.load(Ordering::SeqCst) {
							match receiver.recv_async().await {
								Ok(event) => match event {
									ServiceEvent::ServiceResolved(info) => {
										if let Some(device) =
											resolve_discovered_device(&service_type, &info)
										{
											let key = device_key(&info, &device.address);
											devices.lock().await.insert(key, device);
										}
									}
									ServiceEvent::ServiceRemoved(_, fullname) => {
										// Keys are "fullname:addr:port", so a
										// prefix match removes exactly the
										// entries of the departed service.
										let mut map = devices.lock().await;
										map.retain(|key, _| !key.starts_with(&fullname));
									}
									_ => {}
								},
								Err(e) => {
									error!("Error receiving mDNS event: {}", e);
									break;
								}
							}
						}
					});
				}
				Err(e) => {
					error!("Failed to browse for service {}: {}", service_type, e);
				}
			}
		}

		Ok(())

	}

	pub async fn stop_discovery(&self) {

		self.running.store(false, Ordering::SeqCst);
		self.devices.lock().await.clear();
	}


	pub async fn get_devices(&self) -> Result<Vec<DiscoveredDevice>> {
		let devices = self.devices.lock().await;
		let our_host = hostname::get()
			.ok()
			.map(|h| h.to_string_lossy().to_lowercase())
			.unwrap_or_default();

		Ok(devices
			.values()
			.filter(|d| !is_local_device(d, &our_host))
			.cloned()
			.collect())
	}
}

fn device_key(info: &ServiceInfo, addr: &IpAddr) -> String {
	format!("{}:{}:{}", info.get_fullname(), addr, info.get_port())
}

fn resolve_discovered_device(
	service_type: &str,
	info: &ServiceInfo,
) -> Option<DiscoveredDevice> {
	let addresses = info.get_addresses();
	let addr = addresses.iter().next()?;
	let txt_records: HashMap<String, String> = info
		.get_properties()
		.iter()
		.map(|prop| (prop.key().to_string(), prop.val_str().to_string()))
		.collect();

	let display_name = txt_records
		.get("name")
		.or_else(|| txt_records.get("rpNm"))
		.cloned()
		.filter(|n| !n.is_empty())
		.unwrap_or_else(|| friendly_name(info.get_fullname()));

	let service = match service_type {
		"_airplay._tcp.local." => ServiceType::AirPlay,
		"_raop._tcp.local." => ServiceType::Raop,
		"_airdrop._tcp.local." | "_airdrop._udp.local." => ServiceType::AirDrop,
		"_companion-link._tcp.local." => ServiceType::Companion,
		"_device-info._tcp.local." => ServiceType::DeviceInfo,
		"_apple-mobdev2._tcp.local." => ServiceType::IosMobile,
		"_rdlink._tcp.local." => ServiceType::RemoteDevice,
		_ => ServiceType::DeviceInfo,
	};

	Some(DiscoveredDevice {
		name: display_name,
		address: IpAddr::V4(*addr),
		port: info.get_port(),
		service_type: service,
		txt_records,
		rssi: None,
	})
}

fn is_local_device(device: &DiscoveredDevice, our_host: &str) -> bool {
	if our_host.is_empty() {
		return false;
	}
	let name_lc = device.name.to_lowercase();
	if name_lc == our_host || name_lc.contains("airdropd") {
		return true;
	}
	if let Ok(our_ip) = super::util::primary_ipv4() {
		if device.address == IpAddr::V4(our_ip) {
			return true;
		}
	}
	false
}

#[cfg(test)]
mod tests {
	use super::*;

	fn device(name: &str, txt: &[(&str, &str)]) -> DiscoveredDevice {
		DiscoveredDevice {
			name: name.to_string(),
			address: IpAddr::V4(Ipv4Addr::LOCALHOST),
			port: 0,
			service_type: ServiceType::AirDrop,
			txt_records: txt
				.iter()
				.map(|(k, v)| (k.to_string(), v.to_string()))
				.collect(),
			rssi: None,
		}
	}

	#[test]
	fn model_imac_is_distinct_from_mac_desktop() {
		assert_eq!(
			device("Studio", &[("model", "iMac21,1")]).kind(),
			DeviceKind::IMac
		);
		assert_eq!(
			device("Mini", &[("model", "Macmini9,1")]).kind(),
			DeviceKind::MacDesktop
		);
	}

	#[test]
	fn ipad_uses_distinct_icon_from_iphone() {
		assert_eq!(DeviceKind::IPad.emoji(), "📲");
		assert_eq!(DeviceKind::IPhone.emoji(), "📱");
	}

	#[test]
	fn accessory_label_maps_airpods_and_airtag() {
		assert_eq!(
			device(
				"Find My device A1B2",
				&[("accessory_label", "Find My device")]
			)
			.kind(),
			DeviceKind::AirTag
		);
		assert_eq!(
			device("Pods", &[("accessory_label", "AirPods")]).kind(),
			DeviceKind::AirPods
		);
	}

	#[test]
	fn anonymous_apple_presence_defaults_to_iphone() {
		assert_eq!(
			device("Apple device 1A2B", &[("anonymous", "1"), ("apple_presence", "1")]).kind(),
			DeviceKind::IPhone
		);
	}

	#[test]
	fn android_phone_from_platform_hint() {
		assert_eq!(
			device("Pixel 8", &[("platform", "android"), ("device_class", "phone")]).kind(),
			DeviceKind::AndroidPhone
		);
		assert_eq!(
			device("Galaxy Tab", &[("platform", "android")]).kind(),
			DeviceKind::AndroidTablet
		);
	}

	#[test]
	fn android_ble_manufacturer_detection() {
		assert_eq!(
			android_class_from_ble("Galaxy S24", [0x0075]),
			Some("phone")
		);
		assert_eq!(
			android_class_from_ble("Galaxy Watch", [0x0075]),
			Some("watch")
		);
	}

	#[test]
	fn ble_mobile_profile_detects_anonymous_android_phone() {
		let profile = ble_mobile_profile(
			"",
			&[(0x0075, vec![0x01, 0x02])].into_iter().collect(),
			&HashMap::new(),
			&[],
			None,
			false,
			false,
			false,
		);
		assert!(profile.is_mobile);
		assert_eq!(profile.platform, Some(BleMobilePlatform::Android));
		assert_eq!(profile.device_class, "phone");
	}

	#[test]
	fn ble_mobile_profile_detects_apple_presence_without_name() {
		let profile = ble_mobile_profile(
			"",
			&HashMap::new(),
			&HashMap::new(),
			&[],
			None,
			true,
			false,
			false,
		);
		assert!(profile.is_mobile);
		assert_eq!(profile.platform, Some(BleMobilePlatform::Apple));
	}

	#[test]
	fn ble_mobile_profile_detects_phone_ble_class() {
		let profile = ble_mobile_profile(
			"",
			&HashMap::new(),
			&HashMap::new(),
			&[],
			Some(0x0200), // major class phone
			false,
			false,
			false,
		);
		assert!(profile.is_mobile);
		assert_eq!(profile.device_class, "phone");
	}

	#[test]
	fn anonymous_display_title_for_mobile_ble() {
		let d = device(
			"iPhone nearby A1B2",
			&[("anonymous", "1"), ("apple_presence", "1"), ("mobile_presence", "1")],
		);
		assert_eq!(d.display_title(), "iPhone nearby (A1B2)");
	}

	#[test]
	fn device_filter_matches_platform() {
		let iphone = device("Chris's iPhone", &[("model", "iPhone14,2")]);
		let android = device(
			"Pixel 8",
			&[("platform", "android"), ("device_class", "phone")],
		);
		let tag = device(
			"Find My device A1B2",
			&[("accessory_label", "Find My device")],
		);
		let airpods = device(
			"AirPods A1B2",
			&[("accessory_label", "AirPods")],
		);
		assert!(iphone.matches_filter(crate::config::DeviceFilter::Apple, false));
		assert!(!iphone.matches_filter(crate::config::DeviceFilter::Android, false));
		assert!(android.matches_filter(crate::config::DeviceFilter::Android, false));
		assert!(tag.matches_filter(crate::config::DeviceFilter::AirTags, false));
		assert!(!airpods.matches_filter(crate::config::DeviceFilter::All, false));
		assert!(airpods.matches_filter(crate::config::DeviceFilter::All, true));
	}

	#[test]
	fn everyone_filter_hides_accessories_by_default() {
		let tag = device(
			"AirTag ABCD",
			&[("accessory_label", "Find My device")],
		);
		assert!(!tag.matches_filter(crate::config::DeviceFilter::All, false));
		assert!(tag.matches_filter(crate::config::DeviceFilter::AirTags, false));
	}
}
