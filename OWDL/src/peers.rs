//! AWDL peer table.

use chrono::{DateTime, Utc};
use hashbrown::HashMap;
use std::net::Ipv4Addr;

use crate::constants::PEER_TIMEOUT_SECS;

#[derive(Debug, Clone)]
pub struct PeerEntry {
    pub mac: [u8; 6],
    pub name: String,
    pub ipv4: Option<Ipv4Addr>,
    pub last_seen: DateTime<Utc>,
    pub sequence: u64,
}

#[derive(Debug, Default)]
pub struct PeerTable {
    peers: HashMap<[u8; 6], PeerEntry>,
}

impl PeerTable {
    pub fn upsert(&mut self, mac: [u8; 6], name: String, ipv4: Option<Ipv4Addr>, sequence: u64) {
        self.peers.insert(
            mac,
            PeerEntry {
                mac,
                name,
                ipv4,
                last_seen: Utc::now(),
                sequence,
            },
        );
    }

    pub fn touch(&mut self, mac: &[u8; 6]) {
        if let Some(peer) = self.peers.get_mut(mac) {
            peer.last_seen = Utc::now();
        }
    }

    pub fn prune_stale(&mut self) {
        let cutoff = Utc::now() - chrono::Duration::seconds(PEER_TIMEOUT_SECS);
        self.peers.retain(|_, p| p.last_seen > cutoff);
    }

    pub fn list(&self) -> Vec<PeerEntry> {
        self.peers.values().cloned().collect()
    }

    pub fn get(&self, mac: &[u8; 6]) -> Option<&PeerEntry> {
        self.peers.get(mac)
    }

    pub fn len(&self) -> usize {
        self.peers.len()
    }
}
