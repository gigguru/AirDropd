use mdns_sd::{ServiceEvent, ServiceInfo};
use std::sync::Arc;
use tokio::sync::Mutex;
use anyhow::Result;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr, Ipv4Addr};
use tracing::{info, error, warn};
use std::sync::atomic::{AtomicBool, Ordering};
use socket2::{Socket, Domain, Type, Protocol};
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
	MacDesktop,
	AppleWatch,
	AppleTv,
	WindowsPc,
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
			DeviceKind::MacDesktop => "Mac",
			DeviceKind::AppleWatch => "Apple Watch",
			DeviceKind::AppleTv => "Apple TV",
			DeviceKind::WindowsPc => "Windows PC",
			DeviceKind::Unknown => "Apple device",
		}
	}

	/// Icon used on the radar and in device lists.
	pub fn emoji(&self) -> &'static str {
		match self {
			DeviceKind::IPhone => "📱",
			DeviceKind::IPad => "📱",
			DeviceKind::IPod => "🎵",
			DeviceKind::MacBook => "💻",
			DeviceKind::MacDesktop => "🖥",
			DeviceKind::AppleWatch => "⌚",
			DeviceKind::AppleTv => "📺",
			DeviceKind::WindowsPc => "🪟",
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
	if ml.starts_with("imac")
		|| ml.starts_with("macmini")
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

impl DiscoveredDevice {
	/// Best-effort device category from TXT records, falling back to the name.
	pub fn kind(&self) -> DeviceKind {
		// Hardware identifiers from _device-info (model), companion-link
		// (rpMd), and AirPlay (model) advertisements.
		for key in ["model", "rpMd", "am", "usb_mdl"] {
			if let Some(kind) = self
				.txt_records
				.get(key)
				.and_then(|model| kind_from_model(model))
			{
				return kind;
			}
		}

		let name = self.name.to_ascii_lowercase();
		if name.contains("iphone") {
			DeviceKind::IPhone
		} else if name.contains("ipad") {
			DeviceKind::IPad
		} else if name.contains("macbook") {
			DeviceKind::MacBook
		} else if name.contains("imac")
			|| name.contains("mac mini")
			|| name.contains("mac studio")
			|| name.contains("mac pro")
		{
			DeviceKind::MacDesktop
		} else if name.contains("watch") {
			DeviceKind::AppleWatch
		} else if name.contains("apple tv") {
			DeviceKind::AppleTv
		} else if name.contains("pc") || name.contains("windows") || name.contains("desktop") {
			DeviceKind::WindowsPc
		} else {
			DeviceKind::Unknown
		}
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
										let mut map = devices.lock().await;
										map.retain(|_, d| {
											!fullname.contains(&d.name)
												&& !d
													.txt_records
													.get("name")
													.map(|n| fullname.contains(n))
													.unwrap_or(false)
										});
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
