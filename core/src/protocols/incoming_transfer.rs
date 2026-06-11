//! Incoming AirDrop /Ask prompt — blocks until the user accepts or rejects.

use anyhow::{Context, Result};
use plist::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, oneshot};
use tracing::{info, warn};

use super::apple_plist;

#[derive(Debug, Clone)]
pub struct IncomingFileInfo {
    pub name: String,
    pub size: u64,
    pub file_type: String,
}

#[derive(Debug, Clone)]
pub struct IncomingTransferDetails {
    pub sender_name: String,
    pub sender_model: String,
    pub files: Vec<IncomingFileInfo>,
    /// URL being shared instead of (or alongside) files.
    pub link: Option<String>,
}

impl IncomingTransferDetails {
    pub fn summary(&self) -> String {
        if let Some(link) = &self.link {
            if self.files.is_empty() {
                return format!("{} wants to share a link: {}", self.sender_name, link);
            }
        }
        match self.files.len() {
            0 => format!("{} wants to share files", self.sender_name),
            1 => format!(
                "{} wants to send \"{}\"",
                self.sender_name, self.files[0].name
            ),
            n => format!(
                "{} wants to send {} files",
                self.sender_name, n
            ),
        }
    }

    pub fn total_bytes(&self) -> u64 {
        self.files.iter().map(|f| f.size).sum()
    }
}

struct PendingAsk {
    details: IncomingTransferDetails,
    response: oneshot::Sender<bool>,
}

pub struct IncomingTransferService {
    pending: Arc<Mutex<Option<PendingAsk>>>,
    auto_accept: Arc<AtomicBool>,
}

impl IncomingTransferService {
    pub fn new(auto_accept: bool) -> Self {
        Self {
            pending: Arc::new(Mutex::new(None)),
            auto_accept: Arc::new(AtomicBool::new(auto_accept)),
        }
    }

    pub fn set_auto_accept(&self, enabled: bool) {
        self.auto_accept.store(enabled, Ordering::Relaxed);
    }

    pub async fn peek(&self) -> Option<IncomingTransferDetails> {
        self.pending
            .lock()
            .await
            .as_ref()
            .map(|p| p.details.clone())
    }

    pub async fn respond(&self, accept: bool) {
        if let Some(pending) = self.pending.lock().await.take() {
            let _ = pending.response.send(accept);
        }
    }

    /// Block until the user accepts/rejects or the request times out.
    pub async fn wait_for_decision(&self, details: IncomingTransferDetails) -> bool {
        if self.auto_accept.load(Ordering::Relaxed) {
            info!("Auto-accepting incoming transfer from {}", details.sender_name);
            return true;
        }

        let (tx, rx) = oneshot::channel();
        {
            let mut guard = self.pending.lock().await;
            if guard.is_some() {
                warn!(
                    "Replacing pending transfer prompt with new request from {}",
                    details.sender_name
                );
            }
            *guard = Some(PendingAsk {
                details: details.clone(),
                response: tx,
            });
        }

        info!(
            "Waiting for user decision on transfer from {} ({} file(s))",
            details.sender_name,
            details.files.len()
        );

        match tokio::time::timeout(Duration::from_secs(120), rx).await {
            Ok(Ok(accepted)) => {
                info!(
                    "User {} transfer from {}",
                    if accepted { "accepted" } else { "rejected" },
                    details.sender_name
                );
                accepted
            }
            Ok(Err(_)) => {
                warn!("Transfer prompt channel dropped for {}", details.sender_name);
                false
            }
            Err(_) => {
                warn!("Transfer prompt timed out for {}", details.sender_name);
                self.pending.lock().await.take();
                false
            }
        }
    }
}

pub fn parse_ask_request(body: &[u8]) -> Result<IncomingTransferDetails> {
    let value = apple_plist::parse_plist(body)?;
    let dict = value
        .as_dictionary()
        .context("Ask request is not a plist dictionary")?;

    let sender_name = dict
        .get("SenderComputerName")
        .and_then(|v| v.as_string())
        .unwrap_or("Unknown device")
        .to_string();
    let sender_model = dict
        .get("SenderModelName")
        .and_then(|v| v.as_string())
        .unwrap_or("Apple device")
        .to_string();

    let mut files = Vec::new();
    if let Some(Value::Array(entries)) = dict.get("Files") {
        for entry in entries {
            if let Some(file_dict) = entry.as_dictionary() {
                let name = file_dict
                    .get("FileName")
                    .and_then(|v| v.as_string())
                    .unwrap_or("file")
                    .to_string();
                let size = file_dict
                    .get("FileSize")
                    .and_then(|v| v.as_signed_integer())
                    .unwrap_or(0) as u64;
                let file_type = file_dict
                    .get("FileType")
                    .and_then(|v| v.as_string())
                    .unwrap_or("public.content")
                    .to_string();
                files.push(IncomingFileInfo {
                    name,
                    size,
                    file_type,
                });
            }
        }
    }

    // URL shares arrive as an Items array of strings with no Files.
    let link = dict.get("Items").and_then(|v| match v {
        Value::Array(items) => items.iter().find_map(|item| {
            item.as_string()
                .filter(|s| s.starts_with("http://") || s.starts_with("https://"))
                .map(str::to_string)
        }),
        Value::String(s) if s.starts_with("http") => Some(s.clone()),
        _ => None,
    });

    Ok(IncomingTransferDetails {
        sender_name,
        sender_model,
        files,
        link,
    })
}

pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.0} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}
