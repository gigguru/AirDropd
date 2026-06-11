//! Windows Firewall audit and exception helpers for AirDropd.

use anyhow::{Context, Result};
use std::path::PathBuf;
#[cfg(windows)]
use std::process::Command;
#[cfg(windows)]
use tracing::{info, warn};

/// A network port AirDropd relies on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FirewallPort {
    pub protocol: &'static str,
    pub port: u16,
    pub label: &'static str,
}

/// Ports checked when Windows Firewall may block AirDropd.
pub const AIRDROP_PORTS: &[FirewallPort] = &[
    FirewallPort {
        protocol: "TCP",
        port: 8770,
        label: "AirDrop file transfers (HTTPS)",
    },
    FirewallPort {
        protocol: "TCP",
        port: 8771,
        label: "Web Drop (QR phone uploads)",
    },
    FirewallPort {
        protocol: "UDP",
        port: 5353,
        label: "Device discovery (mDNS)",
    },
    FirewallPort {
        protocol: "TCP",
        port: 7000,
        label: "AirDrop legacy compatibility",
    },
    FirewallPort {
        protocol: "UDP",
        port: 5356,
        label: "Peer discovery (OWDL)",
    },
];

const APP_RULE_NAME: &str = "AirDropd";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FirewallPromptResult {
    /// No action required (firewall off, rules present, or non-Windows).
    NotNeeded,
    /// User chose not to add exceptions.
    Declined,
    /// Exceptions were added successfully.
    Added,
    /// User agreed but adding rules failed (often needs Administrator).
    Failed(String),
}

#[derive(Debug, Clone)]
pub struct FirewallAudit {
    pub firewall_enabled: bool,
    pub missing_ports: Vec<FirewallPort>,
}

impl FirewallAudit {
    pub fn needs_prompt(&self) -> bool {
        self.firewall_enabled && !self.missing_ports.is_empty()
    }
}

/// Check whether Windows Firewall is enabled and AirDropd rules are missing.
pub fn audit() -> FirewallAudit {
    #[cfg(windows)]
    {
        let firewall_enabled = is_firewall_enabled();
        let missing_ports = if firewall_enabled {
            missing_port_rules()
        } else {
            Vec::new()
        };
        return FirewallAudit {
            firewall_enabled,
            missing_ports,
        };
    }

    #[cfg(not(windows))]
    FirewallAudit {
        firewall_enabled: false,
        missing_ports: Vec::new(),
    }
}

/// Show a native message box (Windows) listing blocked ports and ask to add exceptions.
pub fn prompt_and_fix(audit: &FirewallAudit) -> FirewallPromptResult {
    if !audit.needs_prompt() {
        return FirewallPromptResult::NotNeeded;
    }

    #[cfg(windows)]
    {
        let approved = show_native_prompt(&audit.missing_ports);
        if !approved {
            info!("User declined Windows Firewall exceptions");
            return FirewallPromptResult::Declined;
        }

        match add_firewall_exceptions() {
            Ok(()) => {
                info!("Windows Firewall exceptions added for AirDropd");
                FirewallPromptResult::Added
            }
            Err(e) => {
                warn!("Failed to add firewall exceptions: {}", e);
                FirewallPromptResult::Failed(e.to_string())
            }
        }
    }

    #[cfg(not(windows))]
    {
        let _ = audit;
        FirewallPromptResult::NotNeeded
    }
}

pub fn format_port_list(ports: &[FirewallPort]) -> String {
    ports
        .iter()
        .map(|p| format!("• {} {} — {}", p.protocol, p.port, p.label))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(windows)]
fn show_native_prompt(ports: &[FirewallPort]) -> bool {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::UI::WindowsAndMessaging::{
        MessageBoxW, MB_ICONINFORMATION, MB_TOPMOST, MB_YESNO, IDYES,
    };

    let port_list = format_port_list(ports);
    let message = format!(
        "AirDropd needs network access to discover Apple devices and receive files.\n\n\
         Windows Firewall may be blocking the following ports:\n\n\
         {port_list}\n\n\
         Would you like AirDropd to add Windows Firewall exceptions for these ports?\n\n\
         You may be prompted for administrator permission."
    );

    let title = "AirDropd — Firewall access needed";
    let message_wide: Vec<u16> = OsStr::new(&message)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let title_wide: Vec<u16> = OsStr::new(title)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let result = unsafe {
        MessageBoxW(
            None,
            PCWSTR(message_wide.as_ptr()),
            PCWSTR(title_wide.as_ptr()),
            MB_YESNO | MB_ICONINFORMATION | MB_TOPMOST,
        )
    };

    result == IDYES
}

#[cfg(windows)]
pub fn add_firewall_exceptions() -> Result<()> {
    let exe = current_exe()?;
    let exe_str = exe.to_string_lossy();

    // Application rule — allows inbound traffic to AirDropd.exe
    run_netsh(&[
        "advfirewall",
        "firewall",
        "add",
        "rule",
        &format!("name={APP_RULE_NAME}"),
        "dir=in",
        "action=allow",
        &format!("program={exe_str}"),
        "enable=yes",
        "profile=any",
    ])?;

    for port in AIRDROP_PORTS {
        let rule_name = format!("{APP_RULE_NAME} {} {}", port.protocol, port.port);
        if rule_exists(&rule_name) {
            continue;
        }
        run_netsh(&[
            "advfirewall",
            "firewall",
            "add",
            "rule",
            &format!("name={rule_name}"),
            "dir=in",
            "action=allow",
            &format!("protocol={}", port.protocol),
            &format!("localport={}", port.port),
            "enable=yes",
            "profile=any",
        ])?;
    }

    Ok(())
}

#[cfg(windows)]
fn missing_port_rules() -> Vec<FirewallPort> {
    if rule_exists(APP_RULE_NAME) {
        return Vec::new();
    }

    AIRDROP_PORTS
        .iter()
        .copied()
        .filter(|port| {
            let name = format!("{APP_RULE_NAME} {} {}", port.protocol, port.port);
            !rule_exists(&name)
        })
        .collect()
}

#[cfg(windows)]
fn is_firewall_enabled() -> bool {
    let output = Command::new("netsh")
        .args(["advfirewall", "show", "allprofiles", "state"])
        .output();

    match output {
        Ok(out) => {
            let text = String::from_utf8_lossy(&out.stdout).to_ascii_lowercase();
            text.contains("state") && text.contains("on")
        }
        Err(_) => true,
    }
}

#[cfg(windows)]
fn rule_exists(name: &str) -> bool {
    let output = Command::new("netsh")
        .args([
            "advfirewall",
            "firewall",
            "show",
            "rule",
            &format!("name={name}"),
        ])
        .output();

    match output {
        Ok(out) => {
            let text = String::from_utf8_lossy(&out.stdout);
            !text.contains("No rules match")
                && !text.contains("No rules match the specified criteria")
                && text.contains("Rule Name")
        }
        Err(_) => false,
    }
}

#[cfg(windows)]
fn run_netsh(args: &[&str]) -> Result<()> {
    let output = Command::new("netsh")
        .args(args)
        .output()
        .context("failed to run netsh")?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    Err(anyhow::anyhow!(
        "netsh failed: {}{}",
        stderr.trim(),
        if stdout.trim().is_empty() {
            String::new()
        } else {
            format!(" ({})", stdout.trim())
        }
    ))
}

fn current_exe() -> Result<PathBuf> {
    std::env::current_exe().context("could not determine AirDropd executable path")
}

#[cfg(not(windows))]
pub fn add_firewall_exceptions() -> Result<()> {
    Ok(())
}
