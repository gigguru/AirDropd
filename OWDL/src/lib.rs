//! Open Wireless Direct Link (OWDL)
//!
//! Rust AWDL protocol layer used by AirDropd. This crate provides the API
//! surface expected by the application; full AWDL frame handling can be
//! extended here as platform support matures.

pub mod daemon;

use bytes::Bytes;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use daemon::{DaemonBuilder, DaemonConfig, DaemonStats, IoConfig, ServiceConfig};

/// OWDL error type
#[derive(Debug, Error)]
pub enum OwdlError {
    #[error("daemon not initialized")]
    NotInitialized,
    #[error("daemon already running")]
    AlreadyRunning,
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
#[derive(Debug)]
pub struct AwdlDaemon {
    running: bool,
    stats: DaemonStats,
}

impl AwdlDaemon {
    pub async fn init(&mut self) -> OwdlResult<()> {
        tracing::debug!("OWDL daemon initialized (stub)");
        Ok(())
    }

    pub async fn start(&mut self) -> OwdlResult<()> {
        if self.running {
            return Err(OwdlError::AlreadyRunning);
        }
        self.running = true;
        tracing::info!("OWDL daemon started (stub — AWDL requires compatible Wi-Fi hardware)");
        Ok(())
    }

    pub async fn stop(&mut self) -> OwdlResult<()> {
        self.running = false;
        tracing::info!("OWDL daemon stopped");
        Ok(())
    }

    pub async fn get_stats(&self) -> DaemonStats {
        self.stats.clone()
    }
}
