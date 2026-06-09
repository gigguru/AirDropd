//! AWDL daemon builder and configuration

use crate::{AwdlDaemon, OwdlError, OwdlResult};
use serde::{Deserialize, Serialize};

/// Daemon runtime configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DaemonConfig {
    pub enabled: bool,
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
        Self::default()
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

    pub async fn build(self) -> OwdlResult<AwdlDaemon> {
        let _ = (&self.config, &self.io_config, &self.service_config, &self.interface);

        Ok(AwdlDaemon {
            running: false,
            stats: DaemonStats::default(),
        })
    }
}
