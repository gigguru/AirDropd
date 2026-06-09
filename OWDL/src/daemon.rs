//! AWDL daemon builder, configuration, and runtime.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, RwLock, watch};

use crate::peers::PeerTable;
use crate::transport::{self, InfraTransport, TransportEvent};
use crate::{AwdlData, AwdlPeer, OwdlError, OwdlResult};

/// Daemon runtime configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DaemonConfig {
    pub enabled: bool,
    pub device_name: String,
}

impl DaemonConfig {
    pub fn with_device_name(name: impl Into<String>) -> Self {
        Self {
            enabled: true,
            device_name: name.into(),
        }
    }
}

/// I/O configuration for frame capture/injection
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IoConfig {
    pub interface: Option<String>,
}

/// Service discovery configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub service_name: Option<String>,
}

/// Runtime statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DaemonStats {
    pub frames_sent: u64,
    pub frames_received: u64,
    pub peers_discovered: u64,
    pub transport_mode: String,
}

/// Builder for the AWDL daemon
#[derive(Debug, Default)]
pub struct DaemonBuilder {
    config: DaemonConfig,
    io_config: IoConfig,
    service_config: ServiceConfig,
    interface: Option<String>,
}

impl DaemonBuilder {
    pub fn new() -> Self {
        Self {
            config: DaemonConfig {
                enabled: true,
                device_name: "AirDropd".to_string(),
            },
            ..Default::default()
        }
    }

    pub fn with_config(mut self, config: DaemonConfig) -> Self {
        self.config = config;
        self
    }

    pub fn with_io_config(mut self, io_config: IoConfig) -> Self {
        self.io_config = io_config;
        self
    }

    pub fn with_service_config(mut self, service_config: ServiceConfig) -> Self {
        self.service_config = service_config;
        self
    }

    pub fn with_interface(mut self, interface: Option<String>) -> Self {
        self.interface = interface;
        self
    }

    pub async fn build(self) -> OwdlResult<crate::AwdlDaemon> {
        let _ = (&self.io_config, &self.service_config, &self.interface);

        if !self.config.enabled {
            return Ok(crate::AwdlDaemon::disabled());
        }

        let device_name = if self.config.device_name.is_empty() {
            "AirDropd".to_string()
        } else {
            self.config.device_name.clone()
        };

        let local_mac = transport::derive_local_mac(&device_name);
        let local_ipv4 = transport::local_ipv4()
            .map_err(|e| OwdlError::Network(e.to_string()))?;

        let infra = InfraTransport::new(device_name, local_mac, local_ipv4)
            .await
            .map_err(|e| OwdlError::Network(e.to_string()))?;

        Ok(crate::AwdlDaemon::new(
            infra,
            local_mac,
            local_ipv4,
            "infrastructure-udp".to_string(),
        ))
    }
}

/// Internal runtime handle shared by daemon tasks.
pub(crate) struct DaemonRuntime {
    pub transport: Arc<InfraTransport>,
    pub local_mac: [u8; 6],
    pub local_ipv4: std::net::Ipv4Addr,
    pub peers: Arc<Mutex<PeerTable>>,
    pub stats: Arc<RwLock<DaemonStats>>,
    pub shutdown: watch::Sender<bool>,
}

impl DaemonRuntime {
    pub async fn start_tasks(self: Arc<Self>) {
        let transport = self.transport.clone();
        let shutdown_rx = self.shutdown.subscribe();
        tokio::spawn(async move {
            transport.run_rx(shutdown_rx).await;
        });

        let transport_tx = self.transport.clone();
        let shutdown_tx = self.shutdown.subscribe();
        tokio::spawn(async move {
            transport_tx.run_tx(shutdown_tx).await;
        });

        let mut events = self.transport.subscribe();
        let peers = self.peers.clone();
        let stats = self.stats.clone();
        let mut shutdown = self.shutdown.subscribe();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = shutdown.changed() => {
                        if *shutdown.borrow() { break; }
                    }
                    msg = events.recv() => {
                        match msg {
                            Ok(TransportEvent::PeerDiscovered { mac, name, ipv4, sequence }) => {
                                peers.lock().await.upsert(mac, name, ipv4, sequence);
                                stats.write().await.peers_discovered += 1;
                            }
                            Ok(TransportEvent::DataReceived { src_mac, payload }) => {
                                peers.lock().await.touch(&src_mac);
                                stats.write().await.frames_received += 1;
                                tracing::debug!(
                                    "AWDL data from {:02x?}: {} bytes",
                                    src_mac,
                                    payload.len()
                                );
                            }
                            Err(_) => {}
                        }
                    }
                }
            }
        });

        let peers_prune = self.peers.clone();
        let mut shutdown_prune = self.shutdown.subscribe();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = shutdown_prune.changed() => {
                        if *shutdown_prune.borrow() { break; }
                    }
                    _ = tokio::time::sleep(tokio::time::Duration::from_secs(5)) => {
                        peers_prune.lock().await.prune_stale();
                    }
                }
            }
        });
    }

    pub async fn send_data(&self, data: &AwdlData) -> OwdlResult<()> {
        self.transport
            .send_data(data.destination, data.payload.clone())
            .await
            .map_err(|e| OwdlError::Network(e.to_string()))?;
        self.stats.write().await.frames_sent += 1;
        Ok(())
    }

    pub async fn peers(&self) -> Vec<AwdlPeer> {
        self.peers
            .lock()
            .await
            .list()
            .into_iter()
            .map(|p| AwdlPeer {
                address: p.mac,
                name: Some(p.name),
                ipv4: p.ipv4,
                last_seen: p.last_seen,
            })
            .collect()
    }
}
