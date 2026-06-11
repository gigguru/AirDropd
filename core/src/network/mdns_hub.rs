//! Shared mDNS daemon used for both browsing and registration.

use anyhow::Result;
use mdns_sd::ServiceDaemon;
use std::sync::Arc;

pub type SharedMdns = Arc<ServiceDaemon>;

pub fn create_shared_mdns() -> Result<SharedMdns> {
    let daemon = ServiceDaemon::new()?;
    Ok(Arc::new(daemon))
}
