use std::net::{IpAddr, Ipv4Addr};
use anyhow::Result;
use local_ip_address::list_afinet_netifas;

/// Best-effort primary IPv4 for mDNS registration (non-loopback, non-link-local).
pub fn primary_ipv4() -> Result<Ipv4Addr> {
    let interfaces = list_afinet_netifas()?;
    for (_, ip) in interfaces {
        if let IpAddr::V4(addr) = ip {
            if !addr.is_loopback() && !addr.is_link_local() && !addr.is_multicast() {
                return Ok(addr);
            }
        }
    }
    Err(anyhow::anyhow!("No suitable IPv4 address for mDNS"))
}

/// Turn `My iPhone._airdrop._tcp.local.` into `My iPhone`.
pub fn friendly_name(fullname: &str) -> String {
    let base = fullname.split('.').next().unwrap_or(fullname);
    base.replace("\\032", " ")
        .replace("%20", " ")
}
