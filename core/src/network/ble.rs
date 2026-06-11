use btleplug::api::{Central, Manager as _, Peripheral, ScanFilter};
use btleplug::platform::{Adapter, Manager};
use anyhow::{Result, anyhow};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::runtime::Handle; 
use tracing::{info, warn, debug};
use uuid::Uuid;
use std::time::Duration;

/// Apple's BLE manufacturer company identifier.
const APPLE_COMPANY_ID: u16 = 0x004C;

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct BleDevice {
    pub id: String,
    /// BLE local name. Apple Continuity beacons never include one, so this
    /// is empty for iPhones/iPads detected purely over Bluetooth.
    pub name: String,
    pub rssi: i16,
    pub manufacturer_data: HashMap<u16, Vec<u8>>,
    pub service_data: HashMap<Uuid, Vec<u8>>,
    pub last_seen: std::time::Instant,
    /// Device broadcasts Apple Continuity manufacturer data.
    pub apple: bool,
    /// Device is broadcasting an AirDrop beacon (type 0x05) right now —
    /// its owner has the share sheet / AirDrop browser open.
    pub airdrop_active: bool,
    /// Accessory category ("AirPods", "Find My device", …) when the device
    /// only emits accessory beacons. Shown when "Show all nearby devices"
    /// is enabled — handy for locating a lost device by signal strength.
    pub accessory_label: Option<&'static str>,
}

/// What an Apple Continuity advertisement tells us about a device.
#[derive(Default)]
struct AppleBeacon {
    is_apple: bool,
    /// type 0x05: actively browsing AirDrop (share sheet open).
    airdrop: bool,
    /// Presence beacons emitted by iPhones/iPads/Macs while idle:
    /// 0x10 Nearby Info, 0x0C Handoff, 0x0F Nearby Action, 0x0E Tethering.
    presence: bool,
    /// Device only emits accessory beacons:
    /// 0x02 iBeacon, 0x07 Proximity Pairing (AirPods), 0x12 Find My (AirTag).
    accessory_only: bool,
    /// Best label for an accessory-only device.
    accessory_label: Option<&'static str>,
}

/// Parse the type-length-value stream inside Apple manufacturer data.
fn parse_apple_beacon(data: &[u8]) -> AppleBeacon {
    let mut beacon = AppleBeacon {
        is_apple: true,
        ..Default::default()
    };
    let mut seen_device = false;
    let mut accessory: Option<&'static str> = None;

    let mut i = 0;
    while i + 1 < data.len() {
        let msg_type = data[i];
        let len = data[i + 1] as usize;
        match msg_type {
            0x05 => {
                beacon.airdrop = true;
                seen_device = true;
            }
            0x10 | 0x0C | 0x0F | 0x0E | 0x0B | 0x0D => {
                // Nearby Info / Handoff / Nearby Action / Tethering /
                // Magic Switch / Tethering Source — phones, tablets, Macs.
                beacon.presence = true;
                seen_device = true;
            }
            0x07 => accessory = Some(accessory.unwrap_or("AirPods")),
            0x12 => {
                // Find My network beacon: AirTags, and any lost/powered-off
                // Apple device in Find My mode.
                accessory = Some("Find My device");
            }
            0x02 => accessory = Some(accessory.unwrap_or("Beacon")),
            0x0A => accessory = Some(accessory.unwrap_or("AirPlay device")),
            _ => {}
        }
        i += 2 + len;
    }

    beacon.accessory_only = accessory.is_some() && !seen_device;
    beacon.accessory_label = if beacon.accessory_only { accessory } else { None };
    beacon
}

pub struct BleManager {
    manager: Manager,
    adapter: Option<Adapter>,
    discovered_devices: Arc<Mutex<HashMap<String, BleDevice>>>,
    is_scanning: Arc<Mutex<bool>>,
    is_advertising: Arc<Mutex<bool>>,
}

impl BleManager {
    pub async fn new() -> Result<Self> {
        info!("Initializing BLE Manager for AirDrop discovery");
        
        let manager = Manager::new().await?;
        
        Ok(Self {
            manager,
            adapter: None,
            discovered_devices: Arc::new(Mutex::new(HashMap::new())),
            is_scanning: Arc::new(Mutex::new(false)),
            is_advertising: Arc::new(Mutex::new(false)),
        })
    }

    pub async fn initialize(&mut self) -> Result<()> {
        info!("Getting BLE adapters...");
        let adapters = self.manager.adapters().await?;
        
        if adapters.is_empty() {
            return Err(anyhow!("No BLE adapters found"));
        }

        // Use the first available adapter
        let adapter = adapters.into_iter().next().unwrap();
        let info = adapter.adapter_info().await?;
        info!("Using BLE adapter: {}", info);
        
        self.adapter = Some(adapter);

        // Scan for nearby Apple devices
        self.start_scanning().await?;
        Ok(())
    }

    pub async fn start_scanning(&self) -> Result<()> {
        let adapter = self.adapter.as_ref()
            .ok_or_else(|| anyhow!("BLE adapter not initialized"))?;

        let mut is_scanning = self.is_scanning.lock().await;
        if *is_scanning {
            return Ok(());
        }

        info!("Starting BLE scan for AirDrop devices...");

        // Scan all peripherals; Apple AirDrop beacons use manufacturer data (0x004C)
        // and often do not include service UUIDs in the advertisement packet.
        let scan_filter = ScanFilter::default();

        adapter.start_scan(scan_filter).await?;
        *is_scanning = true;

        // Start device discovery loop
        let adapter_clone = adapter.clone();
        let devices = self.discovered_devices.clone();
        let scanning_flag = self.is_scanning.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(2));
            
            while *scanning_flag.lock().await {
                interval.tick().await;
                
                match adapter_clone.peripherals().await {
                    Ok(peripherals) => {
                        for peripheral in peripherals {
                            let props = match peripheral.properties().await {
                                Ok(Some(props)) => props,
                                _ => continue,
                            };
                            let device_id = peripheral.id().to_string();

                            // btleplug strips the company-id prefix from
                            // manufacturer data and keys the map with it.
                            let beacon = props
                                .manufacturer_data
                                .get(&APPLE_COMPANY_ID)
                                .map(|data| parse_apple_beacon(data))
                                .unwrap_or_default();

                            let local_name = props.local_name.clone().unwrap_or_default();

                            // Keep all Apple devices — phones/tablets/Macs and
                            // accessories (the UI decides whether accessories
                            // are shown). Drop non-Apple Bluetooth noise.
                            let has_signal = beacon.airdrop
                                || beacon.presence
                                || beacon.accessory_only
                                || !local_name.is_empty();
                            if !beacon.is_apple || !has_signal {
                                continue;
                            }

                            let device = BleDevice {
                                id: device_id.clone(),
                                name: local_name.clone(),
                                rssi: props.rssi.unwrap_or(0),
                                manufacturer_data: props.manufacturer_data,
                                service_data: props.service_data,
                                last_seen: std::time::Instant::now(),
                                apple: beacon.is_apple,
                                airdrop_active: beacon.airdrop,
                                accessory_label: beacon.accessory_label,
                            };

                            // btleplug keeps departed peripherals in its cache
                            // with frozen properties. Only refresh last_seen
                            // when the advertisement actually changed (RSSI
                            // fluctuates on every real beacon), so devices
                            // that left the area age out instead of haunting
                            // the radar forever.
                            let mut devices_lock = devices.lock().await;
                            match devices_lock.get_mut(&device_id) {
                                Some(existing) => {
                                    // A device just opened its share sheet —
                                    // someone is about to AirDrop something.
                                    if device.airdrop_active && !existing.airdrop_active {
                                        crate::activity::log(
                                            crate::activity::Category::Bluetooth,
                                            format!(
                                                "Nearby Apple device opened AirDrop ({} dBm)",
                                                device.rssi
                                            ),
                                        );
                                    }
                                    let changed = existing.rssi != device.rssi
                                        || existing.manufacturer_data
                                            != device.manufacturer_data
                                        || existing.name != device.name;
                                    let last_seen = if changed {
                                        device.last_seen
                                    } else {
                                        existing.last_seen
                                    };
                                    *existing = BleDevice { last_seen, ..device };
                                }
                                None => {
                                    devices_lock.insert(device_id, device);
                                }
                            }
                            drop(devices_lock);
                            debug!(
                                "BLE device: {} apple={} airdrop_active={}",
                                if local_name.is_empty() { "<anonymous>" } else { &local_name },
                                beacon.is_apple,
                                beacon.airdrop
                            );
                        }
                    }
                    Err(e) => {
                        warn!("Error getting BLE peripherals: {}", e);
                    }
                }
            }
        });

        Ok(())
    }

    pub async fn stop_scanning(&self) -> Result<()> {
        let adapter = self.adapter.as_ref()
            .ok_or_else(|| anyhow!("BLE adapter not initialized"))?;

        let mut is_scanning = self.is_scanning.lock().await;
        if !*is_scanning {
            return Ok(());
        }

        info!("Stopping BLE scan...");
        adapter.stop_scan().await?;
        *is_scanning = false;

        Ok(())
    }

    pub async fn start_advertising_with_name(&self, device_name: &str) -> Result<()> {
        let mut is_advertising = self.is_advertising.lock().await;
        if *is_advertising {
            crate::network::ble_advertise::stop();
            *is_advertising = false;
        }
        drop(is_advertising);

        info!(
            "Starting BLE advertising for AirDrop discovery as \"{}\"...",
            device_name
        );

        match crate::network::ble_advertise::start(device_name) {
            Ok(()) => {
                *self.is_advertising.lock().await = true;
                crate::activity::log(
                    crate::activity::Category::Bluetooth,
                    "AirDrop BLE beacon broadcasting (wakes nearby Apple receivers)",
                );
                Ok(())
            }
            Err(e) => {
                warn!(
                    "BLE advertising failed: {} — iPhones may not discover this PC via AirDrop",
                    e
                );
                crate::activity::log(
                    crate::activity::Category::Error,
                    format!("BLE advertising failed: {}", e),
                );
                Err(e)
            }
        }
    }

    pub async fn start_advertising(&self) -> Result<()> {
        let device_name = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "AirDropd".to_string());
        self.start_advertising_with_name(&device_name).await
    }

    pub async fn restart_advertising(&self, device_name: &str) -> Result<()> {
        let mut is_advertising = self.is_advertising.lock().await;
        if *is_advertising {
            crate::network::ble_advertise::stop();
            *is_advertising = false;
        }
        drop(is_advertising);
        self.start_advertising_with_name(device_name).await
    }

    pub async fn stop_advertising(&self) -> Result<()> {
        let mut is_advertising = self.is_advertising.lock().await;
        if !*is_advertising {
            return Ok(());
        }

        info!("Stopping BLE advertising...");
        crate::network::ble_advertise::stop();
        *is_advertising = false;

        Ok(())
    }

    pub async fn get_discovered_devices(&self) -> Vec<BleDevice> {
        let devices = self.discovered_devices.lock().await;
        let now = std::time::Instant::now();

        // Drop devices whose advertisements went quiet (left the area).
        // 45s tolerates slow advertising intervals without ghosting.
        devices.values()
            .filter(|device| now.duration_since(device.last_seen).as_secs() < 45)
            .cloned()
            .collect()
    }

    pub async fn is_scanning(&self) -> bool {
        *self.is_scanning.lock().await
    }

    pub async fn is_advertising(&self) -> bool {
        *self.is_advertising.lock().await
    }
}

impl Drop for BleManager {
    fn drop(&mut self) {
        // Do NOT create or block a new runtime here. This object is often
        // dropped from within an existing Tokio runtime worker thread, and
        // calling Runtime::new().block_on(...) would panic with:
        // "Cannot start a runtime from within a runtime".
        if let Some(adapter) = self.adapter.clone() {
            let scanning = self.is_scanning.clone();
            let advertising = self.is_advertising.clone();

            if let Ok(handle) = Handle::try_current() {
                // Best-effort async cleanup on the existing runtime.
                handle.spawn(async move {
                    // Stop scanning if it is still running
                    if *scanning.lock().await {
                        let _ = adapter.stop_scan().await;
                        let mut s = scanning.lock().await;
                        *s = false;
                    }

                    // Advertising isn't implemented on Windows in this layer,
                    // but ensure the flag is cleared to keep internal state consistent.
                    let mut adv = advertising.lock().await;
                    *adv = false;
                });
            } else {
                // No runtime available; skip cleanup to avoid panics.
                // OS / driver will clean up scanning resources when the process exits.
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse_apple_beacon;

    /// Idle iPhones broadcast Nearby Info (0x10) — must register as presence.
    #[test]
    fn nearby_info_beacon_is_device_presence() {
        let data = [0x10, 0x05, 0x01, 0x18, 0x12, 0x34, 0x56];
        let beacon = parse_apple_beacon(&data);
        assert!(beacon.is_apple);
        assert!(beacon.presence);
        assert!(!beacon.airdrop);
        assert!(!beacon.accessory_only);
    }

    /// Share-sheet-open devices broadcast the AirDrop TLV (0x05).
    #[test]
    fn airdrop_beacon_is_detected() {
        let mut data = vec![0x05, 0x12];
        data.extend_from_slice(&[0u8; 18]);
        let beacon = parse_apple_beacon(&data);
        assert!(beacon.airdrop);
        assert!(!beacon.accessory_only);
    }

    /// AirPods (proximity pairing 0x07) classify as accessories with a label.
    #[test]
    fn airpods_beacon_is_accessory_only() {
        let data = [0x07, 0x19, 0x01, 0x0E, 0x20, 0x00, 0x00];
        let beacon = parse_apple_beacon(&data);
        assert!(beacon.accessory_only);
        assert!(!beacon.presence);
        assert_eq!(beacon.accessory_label, Some("AirPods"));
    }

    /// Find My beacons (0x12) — AirTags or lost devices — get the right label.
    #[test]
    fn find_my_beacon_is_labeled_tracker() {
        let data = [0x12, 0x19, 0x10, 0x00, 0x00];
        let beacon = parse_apple_beacon(&data);
        assert!(beacon.accessory_only);
        assert_eq!(beacon.accessory_label, Some("Find My device"));
    }

    /// A device emitting both Handoff and an accessory TLV is still a device.
    #[test]
    fn mixed_beacon_prefers_device_classification() {
        let data = [0x02, 0x02, 0xAA, 0xBB, 0x0C, 0x02, 0x01, 0x02];
        let beacon = parse_apple_beacon(&data);
        assert!(beacon.presence);
        assert!(!beacon.accessory_only);
    }
}
