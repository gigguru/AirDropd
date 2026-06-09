//! Windows BLE advertisement so nearby Apple devices can discover AirDropd.

#[cfg(windows)]
mod imp {
    use anyhow::{Context, Result};
    use std::sync::Mutex;
    use tracing::{info, warn};
    use windows::core::HSTRING;
    use windows::Devices::Bluetooth::Advertisement::{
        BluetoothLEAdvertisementPublisher, BluetoothLEAdvertisementPublisherStatus,
        BluetoothLEManufacturerData, IBluetoothLEAdvertisement,
    };
    use windows::Storage::Streams::DataWriter;

    static PUBLISHER: Mutex<Option<BluetoothLEAdvertisementPublisher>> = Mutex::new(None);

    fn airdrop_manufacturer_payload() -> Result<windows::Storage::Streams::IBuffer> {
        let writer = DataWriter::new()?;
        // Apple company ID (little-endian on air)
        writer.WriteByte(0x4C)?;
        writer.WriteByte(0x00)?;
        // AirDrop / Handoff proximity type
        writer.WriteByte(0x05)?;
        writer.WriteByte(0x01)?; // discoverable
        // 6-byte device hash placeholder
        for _ in 0..6 {
            writer.WriteByte(0x00)?;
        }
        writer.WriteByte(0x00)?;
        writer.WriteByte(0x00)?;
        writer.WriteByte(0x00)?;
        writer.WriteByte(0x00)?;
        writer.DetachBuffer().context("Failed to build BLE payload")
    }

    pub fn start(device_name: &str) -> Result<()> {
        let mut guard = PUBLISHER
            .lock()
            .map_err(|_| anyhow::anyhow!("BLE publisher lock poisoned"))?;

        if guard.is_some() {
            return Ok(());
        }

        let publisher = BluetoothLEAdvertisementPublisher::new()?;
        let advertisement = publisher.Advertisement()?;

        advertisement.SetLocalName(&HSTRING::from(device_name))?;

        let buffer = airdrop_manufacturer_payload()?;
        let mfg = BluetoothLEManufacturerData::Create(0x004C, &buffer)?;
        advertisement.ManufacturerData()?.Append(&mfg)?;

        publisher.Start()?;

        let status = publisher.Status()?;
        if status != BluetoothLEAdvertisementPublisherStatus::Started
            && status != BluetoothLEAdvertisementPublisherStatus::Created
        {
            warn!("BLE publisher status: {:?}", status);
        } else {
            info!("BLE AirDrop advertisement started as \"{}\"", device_name);
        }

        *guard = Some(publisher);
        Ok(())
    }

    pub fn stop() {
        if let Ok(mut guard) = PUBLISHER.lock() {
            if let Some(publisher) = guard.take() {
                let _ = publisher.Stop();
            }
        }
    }
}

#[cfg(windows)]
pub use imp::{start, stop};

#[cfg(not(windows))]
pub fn start(_device_name: &str) -> anyhow::Result<()> {
    Ok(())
}

#[cfg(not(windows))]
pub fn stop() {}
