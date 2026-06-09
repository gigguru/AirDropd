//! AWDL TLV and action frame (PSF) encoding/decoding.

use crate::constants::{AWDL_BSSID, AWDL_OUI_TYPE, AWDL_VERSION, APPLE_OUI};
use crate::wire::{WireReader, WireWriter};
use rand::Rng;

/// TLV type identifiers from OWL frame.h
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TlvType {
    ServiceParameters = 0x01,
    SynchronizationParameters = 0x02,
    ElectionParameters = 0x03,
    ChannelSequence = 0x04,
    DatapathParameters = 0x05,
    Arpa = 0x06,
    Enet = 0x07,
    Unknown = 0xff,
}

impl From<u8> for TlvType {
    fn from(v: u8) -> Self {
        match v {
            0x01 => Self::ServiceParameters,
            0x02 => Self::SynchronizationParameters,
            0x03 => Self::ElectionParameters,
            0x04 => Self::ChannelSequence,
            0x05 => Self::DatapathParameters,
            0x06 => Self::Arpa,
            0x07 => Self::Enet,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Tlv {
    pub ty: TlvType,
    pub value: Vec<u8>,
}

impl Tlv {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = WireWriter::new();
        w.write_u8(self.ty as u8);
        w.write_u16(self.value.len() as u16);
        w.write_bytes(&self.value);
        w.finish()
    }

    pub fn parse_all(data: &[u8]) -> Vec<Tlv> {
        let mut tlvs = Vec::new();
        let mut r = WireReader::new(data);
        while r.remaining() >= 3 {
            let ty = TlvType::from(r.read_u8().unwrap_or(0xff));
            let len = match r.read_u16() {
                Some(l) => l as usize,
                None => break,
            };
            let value = match r.read_bytes(len) {
                Some(v) => v,
                None => break,
            };
            tlvs.push(Tlv { ty, value });
        }
        tlvs
    }
}

/// Periodic Synchronization Frame payload (vendor-specific action frame body).
#[derive(Debug, Clone)]
pub struct PsfFrame {
    pub src_mac: [u8; 6],
    pub device_name: String,
    pub ipv4: Option<std::net::Ipv4Addr>,
    pub sequence: u64,
}

impl PsfFrame {
    pub fn build(src_mac: [u8; 6], device_name: &str, ipv4: Option<std::net::Ipv4Addr>, sequence: u64) -> Vec<u8> {
        let mut rng = rand::thread_rng();
        let mut w = WireWriter::new();

        // Fixed AWDL action frame header
        w.write_u8(0x00); // category: vendor specific
        w.write_u8(0x7f); // action
        w.write_bytes(&APPLE_OUI);
        w.write_u8(AWDL_OUI_TYPE);
        w.write_u16(AWDL_VERSION);
        w.write_u8(0x00); // PSF subtype
        w.write_u64(sequence);
        w.write_u64(sequence + 1);
        w.write_mac(&AWDL_BSSID);

        // Service parameters TLV
        let mut svc = WireWriter::new();
        svc.write_u16(0x0001); // AFM mode
        svc.write_u8(0x01);
        let svc_tlv = Tlv {
            ty: TlvType::ServiceParameters,
            value: svc.finish(),
        };

        // Sync parameters TLV
        let mut sync = WireWriter::new();
        sync.write_u64(sequence);
        sync.write_u32(0x00001000); // availability window
        sync.write_u32(0x00000010);
        let sync_tlv = Tlv {
            ty: TlvType::SynchronizationParameters,
            value: sync.finish(),
        };

        // Election parameters TLV (declare self as master candidate)
        let mut elect = WireWriter::new();
        elect.write_mac(&src_mac);
        elect.write_u32(rng.gen::<u32>() | 0x80000000);
        let elect_tlv = Tlv {
            ty: TlvType::ElectionParameters,
            value: elect.finish(),
        };

        // Channel sequence TLV (social channels)
        let mut chan = WireWriter::new();
        chan.write_u8(0x03); // opclass encoding
        for &(num, opclass) in crate::constants::SOCIAL_CHANNELS.iter() {
            chan.write_u8(num);
            chan.write_u8(opclass);
        }
        let chan_tlv = Tlv {
            ty: TlvType::ChannelSequence,
            value: chan.finish(),
        };

        // Datapath / device name TLV (Enet carries hostname for discovery)
        let mut enet = WireWriter::new();
        enet.write_mac(&src_mac);
        if let Some(ip) = ipv4 {
            enet.write_bytes(&ip.octets());
        }
        let name_bytes = device_name.as_bytes();
        enet.write_u16(name_bytes.len() as u16);
        enet.write_bytes(name_bytes);
        let enet_tlv = Tlv {
            ty: TlvType::Enet,
            value: enet.finish(),
        };

        for tlv in [svc_tlv, sync_tlv, elect_tlv, chan_tlv, enet_tlv] {
            w.write_bytes(&tlv.encode());
        }

        w.finish()
    }

    pub fn parse(payload: &[u8]) -> Option<Self> {
        if payload.len() < 24 {
            return None;
        }
        let mut r = WireReader::new(payload);
        let _cat = r.read_u8()?;
        let _act = r.read_u8()?;
        let oui = r.read_bytes(3)?;
        if oui != APPLE_OUI {
            return None;
        }
        let _oui_type = r.read_u8()?;
        let _version = r.read_u16()?;
        let _subtype = r.read_u8()?;
        let sequence = r.read_u64()?;
        let _tx_time = r.read_u64()?;
        let _bssid = r.read_mac()?;

        let tlv_start = payload.len() - r.remaining();
        let tlvs = Tlv::parse_all(&payload[tlv_start..]);

        let mut src_mac = AWDL_BSSID;
        let mut device_name = String::new();
        let mut ipv4 = None;

        for tlv in tlvs {
            match tlv.ty {
                TlvType::ElectionParameters if tlv.value.len() >= 6 => {
                    src_mac.copy_from_slice(&tlv.value[..6]);
                }
                TlvType::Enet => {
                    let mut tr = WireReader::new(&tlv.value);
                    if let Some(mac) = tr.read_mac() {
                        src_mac = mac;
                    }
                    if tr.remaining() >= 4 {
                        let ip_bytes = tr.read_bytes(4)?;
                        ipv4 = Some(std::net::Ipv4Addr::new(
                            ip_bytes[0], ip_bytes[1], ip_bytes[2], ip_bytes[3],
                        ));
                    }
                    if tr.remaining() >= 2 {
                        let name_len = tr.read_u16()? as usize;
                        if let Some(name) = tr.read_bytes(name_len) {
                            device_name = String::from_utf8_lossy(&name).to_string();
                        }
                    }
                }
                _ => {}
            }
        }

        Some(PsfFrame {
            src_mac,
            device_name,
            ipv4,
            sequence,
        })
    }
}

/// AWDL data frame wrapper for IP payloads.
#[derive(Debug, Clone)]
pub struct DataFrame {
    pub src_mac: [u8; 6],
    pub dst_mac: [u8; 6],
    pub ethertype: u16,
    pub payload: bytes::Bytes,
}

impl DataFrame {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = WireWriter::new();
        w.write_mac(&AWDL_BSSID);
        w.write_mac(&self.src_mac);
        w.write_mac(&self.dst_mac);
        w.write_bytes(&APPLE_OUI);
        w.write_u16(0x0000); // protocol id
        w.write_u16(0x0000); // sequence
        w.write_u16(self.ethertype);
        w.write_bytes(&self.payload);
        w.finish()
    }

    pub fn parse(payload: &[u8]) -> Option<Self> {
        let mut r = WireReader::new(payload);
        let _bssid = r.read_mac()?;
        let _ = r.read_mac()?;
        let src = r.read_mac()?;
        let dst = r.read_mac()?;
        let _ = r.read_bytes(3)?;
        let _ = r.read_u16()?;
        let _ = r.read_u16()?;
        let ethertype = r.read_u16()?;
        let rest = r.read_bytes(r.remaining())?;
        Some(DataFrame {
            src_mac: src,
            dst_mac: dst,
            ethertype,
            payload: bytes::Bytes::from(rest),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn psf_roundtrip() {
        let mac = [0x02, 0x11, 0x22, 0x33, 0x44, 0x55];
        let raw = PsfFrame::build(mac, "AirDropd-PC", Some(std::net::Ipv4Addr::new(192, 168, 1, 10)), 42);
        let parsed = PsfFrame::parse(&raw).expect("parse psf");
        assert_eq!(parsed.src_mac, mac);
        assert_eq!(parsed.device_name, "AirDropd-PC");
        assert_eq!(parsed.ipv4, Some(std::net::Ipv4Addr::new(192, 168, 1, 10)));
    }
}
