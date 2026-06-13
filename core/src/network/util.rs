use std::net::{IpAddr, Ipv4Addr};

use anyhow::Result;
use local_ip_address::list_afinet_netifas;

/// Best-effort primary IPv4 for mDNS registration (non-loopback, non-link-local).
pub fn primary_ipv4() -> Result<Ipv4Addr> {
    best_lan_ipv4()
}

/// Pick the IPv4 address phones on the same Wi-Fi are most likely to reach.
///
/// VPN/tunnel interfaces (utun*) and AWDL are skipped; Wi-Fi (`en0` on Mac) is
/// preferred over Ethernet or virtual adapters.
pub fn best_lan_ipv4() -> Result<Ipv4Addr> {
    let interfaces = list_afinet_netifas()?;
    let mut candidates: Vec<(i32, Ipv4Addr)> = Vec::new();

    for (name, ip) in interfaces {
        let IpAddr::V4(addr) = ip else { continue };
        if !is_usable_lan_ipv4(&addr) {
            continue;
        }
        candidates.push((interface_priority(&name, &addr), addr));
    }

    candidates.sort_by(|a, b| b.0.cmp(&a.0));
    candidates
        .first()
        .map(|(_, ip)| *ip)
        .ok_or_else(|| anyhow::anyhow!("No suitable IPv4 address on Wi-Fi/LAN"))
}

fn is_usable_lan_ipv4(addr: &Ipv4Addr) -> bool {
    !addr.is_loopback()
        && !addr.is_link_local()
        && !addr.is_multicast()
        && !addr.is_broadcast()
        && !addr.is_unspecified()
}

fn interface_priority(name: &str, addr: &Ipv4Addr) -> i32 {
    let n = name.to_ascii_lowercase();
    if n == "en0" {
        return 1_000;
    }
    if n.starts_with("en") && !n.contains("awdl") {
        return 800;
    }
    if n.contains("wifi") || n.contains("wlan") {
        return 700;
    }
    if n.starts_with("utun")
        || n.starts_with("bridge")
        || n.contains("awdl")
        || n == "lo0"
        || n.starts_with("gif")
        || n.starts_with("stf")
        || n.starts_with("llw")
    {
        return -1_000;
    }

    let o = addr.octets();
    if o[0] == 192 && o[1] == 168 {
        return 500;
    }
    if o[0] == 10 {
        return 400;
    }
    if o[0] == 172 && (16..=31).contains(&o[1]) {
        return 300;
    }
    100
}

/// Turn `My iPhone._airdrop._tcp.local.` into `My iPhone`.
pub fn friendly_name(fullname: &str) -> String {
    let base = fullname.split('.').next().unwrap_or(fullname);
    base.replace("\\032", " ")
        .replace("%20", " ")
}
