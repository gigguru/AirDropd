//! Windows BLE advertisement so nearby Apple devices can discover AirDropd.

#[cfg(windows)]
mod imp {
    use anyhow::{Context, Result};
    use std::sync::Mutex;
    use tracing::{info, warn};
    use windows::Devices::Bluetooth::Advertisement::{
        BluetoothLEAdvertisementPublisher, BluetoothLEAdvertisementPublisherStatus,
        BluetoothLEManufacturerData,
    };
    use windows::Storage::Streams::DataWriter;

    static PUBLISHER: Mutex<Option<BluetoothLEAdvertisementPublisher>> = Mutex::new(None);

    /// Apple AirDrop BLE advertisement TLV, byte-exact per Apple's Continuity
    /// protocol (and OpenDrop): `05 12` then 18 bytes — 8 zero padding,
    /// version 0x01, four 2-byte truncated contact hashes, one trailing zero.
    /// Zeroed hashes are what a sender with no contact identity broadcasts;
    /// receivers in "Everyone" mode wake their AirDrop stack on any valid
    /// beacon. A malformed TLV (wrong length byte) is silently ignored by
    /// Apple parsers, so this format must match exactly.
    fn airdrop_manufacturer_payload() -> Result<windows::Storage::Streams::IBuffer> {
        let writer = DataWriter::new()?;
        // Payload only — Windows prepends company ID 0x004C via Create().
        writer.WriteByte(0x05)?; // type: AirDrop
        writer.WriteByte(0x12)?; // length: 18
        for _ in 0..8 {
            writer.WriteByte(0x00)?; // padding
        }
        writer.WriteByte(0x01)?; // version
        for _ in 0..8 {
            writer.WriteByte(0x00)?; // 4 × 2-byte contact hashes (none)
        }
        writer.WriteByte(0x00)?;
        writer.DetachBuffer().context("Failed to build BLE payload")
    }

    pub fn start(device_name: &str) -> Result<()> {
        let mut guard = PUBLISHER
            .lock()
            .map_err(|_| anyhow::anyhow!("BLE publisher lock poisoned"))?;

        if guard.is_some() {
            if let Some(publisher) = guard.take() {
                let _ = publisher.Stop();
            }
        }

        let publisher = BluetoothLEAdvertisementPublisher::new()?;
        let advertisement = publisher.Advertisement()?;

        // No local name: Apple AirDrop beacons never carry one, and the
        // 20-byte TLV plus a name can exceed the 31-byte legacy advertising
        // limit, which makes the Windows publisher fail to start entirely.
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
