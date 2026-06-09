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
