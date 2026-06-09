//! Open Wireless Direct Link (OWDL)
//!
//! Rust AWDL protocol implementation for AirDropd. Uses infrastructure-mode
//! UDP transport on Windows (compatible with same-LAN Apple devices) and can
//! be extended with raw 802.11 via the `raw80211` feature when monitor mode
//! hardware is available.

pub mod channel;
pub mod constants;
pub mod daemon;
pub mod frame;
pub mod peers;
pub mod transport;
pub mod wire;

use bytes::Bytes;
use chrono::{DateTime, Utc};
use daemon::DaemonRuntime;
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;

pub use daemon::{DaemonBuilder, DaemonConfig, DaemonStats, IoConfig, ServiceConfig};

/// OWDL error type
#[derive(Debug, Error)]
pub enum OwdlError {
    #[error("daemon not initialized")]
    NotInitialized,
    #[error("daemon already running")]
    AlreadyRunning,
    #[error("daemon not running")]
    NotRunning,
    #[error("network error: {0}")]
    Network(String),
    #[error("platform not supported")]
    UnsupportedPlatform,
}

pub type OwdlResult<T> = Result<T, OwdlError>;

/// Discovered AWDL peer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwdlPeer {
    pub address: [u8; 6],
    pub name: Option<String>,
    pub ipv4: Option<Ipv4Addr>,
    pub last_seen: DateTime<Utc>,
}

/// AWDL data frame
#[derive(Debug, Clone)]
pub struct AwdlData {
    pub destination: [u8; 6],
    pub source: [u8; 6],
    pub ethertype: u16,
    pub payload: Bytes,
}

impl AwdlData {
    pub fn new(destination: [u8; 6], source: [u8; 6], ethertype: u16, payload: Bytes) -> Self {
        Self {
            destination,
            source,
            ethertype,
            payload,
        }
    }
}

/// AWDL daemon handle
pub struct AwdlDaemon {
    running: bool,
    disabled: bool,
    runtime: Option<Arc<DaemonRuntime>>,
    local_mac: [u8; 6],
    stats: DaemonStats,
}

impl std::fmt::Debug for AwdlDaemon {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AwdlDaemon")
            .field("running", &self.running)
            .field("disabled", &self.disabled)
            .field("local_mac", &self.local_mac)
            .finish()
    }
}

impl AwdlDaemon {
    pub(crate) fn disabled() -> Self {
        Self {
            running: false,
            disabled: true,
            runtime: None,
            local_mac: [0; 6],
            stats: DaemonStats {
                transport_mode: "disabled".to_string(),
                ..Default::default()
            },
        }
    }

    pub(crate) fn new(
        transport: transport::InfraTransport,
        local_mac: [u8; 6],
        local_ipv4: Ipv4Addr,
        mode: String,
    ) -> Self {
        let (shutdown, _) = tokio::sync::watch::channel(false);
        let stats = Arc::new(RwLock::new(DaemonStats {
            transport_mode: mode,
            ..Default::default()
        }));

        let runtime = Arc::new(DaemonRuntime {
            transport: Arc::new(transport),
            local_mac,
            local_ipv4,
            peers: Arc::new(tokio::sync::Mutex::new(peers::PeerTable::default())),
            stats: stats.clone(),
            shutdown,
        });

        Self {
            running: false,
            disabled: false,
            runtime: Some(runtime),
            local_mac,
            stats: DaemonStats::default(),
        }
    }

    pub async fn init(&mut self) -> OwdlResult<()> {
        if self.disabled {
            tracing::info!("OWDL daemon disabled in configuration");
            return Ok(());
        }
        tracing::info!(
            "OWDL daemon initialized (local MAC {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x})",
            self.local_mac[0],
            self.local_mac[1],
            self.local_mac[2],
            self.local_mac[3],
            self.local_mac[4],
            self.local_mac[5],
        );
        Ok(())
    }

    pub async fn start(&mut self) -> OwdlResult<()> {
        if self.disabled {
            return Ok(());
        }
        if self.running {
            return Err(OwdlError::AlreadyRunning);
        }

        if let Some(runtime) = self.runtime.clone() {
            runtime.clone().start_tasks().await;
            self.stats = runtime.stats.read().await.clone();
            self.running = true;
            tracing::info!(
                "OWDL AWDL daemon started ({})",
                self.stats.transport_mode
            );
        }
        Ok(())
    }

    pub async fn stop(&mut self) -> OwdlResult<()> {
        if self.disabled {
            return Ok(());
        }
        if let Some(runtime) = self.runtime.as_ref() {
            let _ = runtime.shutdown.send(true);
        }
        self.running = false;
        tracing::info!("OWDL daemon stopped");
        Ok(())
    }

    pub async fn get_stats(&self) -> DaemonStats {
        if let Some(runtime) = &self.runtime {
            runtime.stats.read().await.clone()
        } else {
            self.stats.clone()
        }
    }

    pub async fn get_peers(&self) -> Vec<AwdlPeer> {
        if let Some(runtime) = &self.runtime {
            runtime.peers().await
        } else {
            Vec::new()
        }
    }

    pub fn local_mac(&self) -> [u8; 6] {
        self.local_mac
    }

    pub async fn send(&self, data: AwdlData) -> OwdlResult<()> {
        if self.disabled {
            return Err(OwdlError::NotInitialized);
        }
        if !self.running {
            return Err(OwdlError::NotRunning);
        }
        if let Some(runtime) = &self.runtime {
            runtime.send_data(&data).await
        } else {
            Err(OwdlError::NotInitialized)
        }
    }
}
