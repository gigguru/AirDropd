use anyhow::{Result, anyhow};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_rustls::TlsAcceptor;
use tracing::{info, error, debug};
use serde_json;
use std::collections::HashMap;
use std::net::SocketAddr;
use rcgen::{Certificate, CertificateParams, DistinguishedName, DnType};
use tokio_rustls::rustls::{Certificate as RustlsCert, PrivateKey as RustlsKey, ServerConfig};
use tokio_rustls::server::TlsStream as RustlsTlsStream;

use crate::config::{self, SharedConfig};

/// HTTP/HTTPS server for AirDrop protocol
pub struct AirDropHttpServer {
    port: u16,
    config: SharedConfig,
    received_tx: Option<tokio::sync::broadcast::Sender<PathBuf>>,
    tls_acceptor: Option<TlsAcceptor>,
    running: Arc<Mutex<bool>>,
}

impl AirDropHttpServer {
    pub fn new(port: u16, config: SharedConfig) -> Self {
        Self {
            port,
            config,
            received_tx: None,
            tls_acceptor: None,
            running: Arc::new(Mutex::new(false)),
        }
    }

    pub fn with_received_notifier(
        mut self,
        tx: tokio::sync::broadcast::Sender<PathBuf>,
    ) -> Self {
        self.received_tx = Some(tx);
        self
    }

    async fn build_rustls_config() -> Result<Arc<ServerConfig>> {
        info!("Generating self-signed certificate for AirDrop HTTPS server (rustls)...");

        let mut params = CertificateParams::new(vec!["AirDrop".to_string(), "AirDropd".to_string()]);
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, "AirDrop");
        dn.push(DnType::OrganizationName, "AirDropd");
        dn.push(DnType::CountryName, "US");
        params.distinguished_name = dn;

        let cert = Certificate::from_params(params)?;
        let cert_der = cert.serialize_der()?;
        let key_der = cert.serialize_private_key_der();

        let cert_chain = vec![RustlsCert(cert_der)];
        let key = RustlsKey(key_der);

        let config = ServerConfig::builder()
            .with_safe_defaults()
            .with_no_client_auth()
            .with_single_cert(cert_chain, key)?;

        Ok(Arc::new(config))
    }

    pub async fn initialize(&mut self) -> Result<()> {
        let tls_config = Self::build_rustls_config().await?;
        let acceptor = TlsAcceptor::from(tls_config);
        self.tls_acceptor = Some(acceptor);
        Ok(())
    }

    pub async fn start(&self) -> Result<()> {
        let acceptor = self
            .tls_acceptor
            .as_ref()
            .ok_or_else(|| anyhow!("TLS acceptor not initialized"))?;

        let listener = TcpListener::bind(("0.0.0.0", self.port)).await?;
        info!("AirDrop HTTPS server listening on port {}", self.port);

        *self.running.lock().await = true;
        let running = self.running.clone();
        let acceptor = acceptor.clone();
        let config = self.config.clone();
        let received_tx = self.received_tx.clone();

        tokio::spawn(async move {
            while *running.lock().await {
                match listener.accept().await {
                    Ok((stream, addr)) => {
                        let acceptor = acceptor.clone();
                        let config = config.clone();
                        let received_tx = received_tx.clone();
                        tokio::spawn(async move {
                            if let Err(e) =
                                Self::handle_connection(stream, addr, acceptor, config, received_tx)
                                    .await
                            {
                                error!("Error handling connection from {}: {}", addr, e);
                            }
                        });
                    }
                    Err(e) => {
                        error!("Error accepting connection: {}", e);
                        break;
                    }
                }
            }
        });

        Ok(())
    }

    async fn handle_connection(
        stream: TcpStream,
        addr: SocketAddr,
        acceptor: TlsAcceptor,
        config: SharedConfig,
        received_tx: Option<tokio::sync::broadcast::Sender<PathBuf>>,
    ) -> Result<()> {
        debug!("Handling HTTPS connection from {}", addr);

        let mut tls_stream = acceptor.accept(stream).await?;
        let mut buffer = Vec::new();
        let mut temp_buf = [0u8; 8192];

        loop {
            let n = tls_stream.read(&mut temp_buf).await?;
            if n == 0 {
                break;
            }
            buffer.extend_from_slice(&temp_buf[..n]);
            if buffer.windows(4).any(|w| w == b"\r\n\r\n") {
                break;
            }
        }

        let request = String::from_utf8_lossy(&buffer);
        let lines: Vec<&str> = request.lines().collect();

        if lines.is_empty() {
            return Err(anyhow!("Empty HTTP request"));
        }

        let request_line = lines[0];
        let parts: Vec<&str> = request_line.split_whitespace().collect();

        if parts.len() < 3 {
            return Err(anyhow!("Invalid HTTP request line"));
        }

        let method = parts[0];
        let path = parts[1];

        debug!("HTTP {} request to {}", method, path);

        match (method, path) {
            ("GET", "/") => Self::handle_root_request(&mut tls_stream).await?,
            ("POST", "/Discover") => {
                Self::handle_discover_request(&mut tls_stream, &config).await?
            }
            ("POST", "/Ask") => Self::handle_ask_request(&mut tls_stream, &config).await?,
            ("POST", "/Upload") => {
                Self::handle_upload_request(&mut tls_stream, &buffer, &config, received_tx)
                    .await?
            }
            _ => Self::handle_not_found(&mut tls_stream).await?,
        }

        Ok(())
    }

    async fn handle_root_request(stream: &mut RustlsTlsStream<TcpStream>) -> Result<()> {
        let response = "HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n";
        stream.write_all(response.as_bytes()).await?;
        Ok(())
    }

    async fn handle_discover_request(
        stream: &mut RustlsTlsStream<TcpStream>,
        config: &SharedConfig,
    ) -> Result<()> {
        info!("Handling /Discover request");

        let broadcast_name = config
            .read()
            .map(|c| c.broadcast_name.clone())
            .unwrap_or_else(|_| config::default_broadcast_name());

        let mut discover_response = HashMap::new();
        discover_response.insert(
            "ReceiverMediaCapabilities",
            serde_json::json!({
                "Version": 1,
                "Vendor": {
                    "com.microsoft": {
                        "OSVersion": [10, 0],
                        "OSBuildVersion": "22000"
                    }
                }
            }),
        );
        discover_response.insert(
            "ReceiverComputerName",
            serde_json::Value::String(broadcast_name.clone()),
        );
        discover_response.insert(
            "ReceiverModelName",
            serde_json::Value::String("Windows,1".to_string()),
        );

        Self::write_json_response(stream, &discover_response).await
    }

    async fn handle_ask_request(
        stream: &mut RustlsTlsStream<TcpStream>,
        config: &SharedConfig,
    ) -> Result<()> {
        info!("Handling /Ask request — accepting transfer");

        let broadcast_name = config
            .read()
            .map(|c| c.broadcast_name.clone())
            .unwrap_or_else(|_| config::default_broadcast_name());

        let ask_response = serde_json::json!({
            "ReceiverModelName": "Windows,1",
            "ReceiverComputerName": broadcast_name,
            "ReceiverMediaCapabilities": {
                "Version": 1,
                "Vendor": {
                    "com.microsoft": {
                        "OSVersion": [10, 0],
                        "OSBuildVersion": "22000"
                    }
                }
            },
            "ConvertTo": ["com.microsoft.windows"],
        });

        Self::write_json_response(stream, &ask_response).await
    }

    async fn handle_upload_request(
        stream: &mut RustlsTlsStream<TcpStream>,
        buffer: &[u8],
        config: &SharedConfig,
        received_tx: Option<tokio::sync::broadcast::Sender<PathBuf>>,
    ) -> Result<()> {
        info!("Handling /Upload request");

        let header_end = buffer
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .ok_or_else(|| anyhow!("Could not find end of HTTP headers"))?;

        let headers = String::from_utf8_lossy(&buffer[..header_end]);
        let filename = parse_filename_from_headers(&headers).unwrap_or_else(|| {
            format!(
                "AirDrop_{}.bin",
                chrono::Utc::now().format("%Y%m%d_%H%M%S")
            )
        });

        let body_start = header_end + 4;
        let mut file_data = buffer[body_start..].to_vec();

        // Read remaining body if Content-Length specified
        if let Some(len) = parse_content_length(&headers) {
            while file_data.len() < len {
                let mut chunk = vec![0u8; 8192];
                let n = stream.read(&mut chunk).await?;
                if n == 0 {
                    break;
                }
                file_data.extend_from_slice(&chunk[..n]);
            }
            file_data.truncate(len);
        }

        let receive_dir = {
            let cfg = config
                .read()
                .map_err(|_| anyhow!("config lock poisoned"))?;
            cfg.ensure_receive_dir()?
        };
        let file_path = config::unique_receive_path(&receive_dir, &filename);

        tokio::fs::write(&file_path, &file_data).await?;
        info!("Saved received file to {:?}", file_path);

        if let Some(tx) = received_tx {
            let _ = tx.send(file_path.clone());
        }

        let response = "HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n";
        stream.write_all(response.as_bytes()).await?;
        Ok(())
    }

    async fn write_json_response(
        stream: &mut RustlsTlsStream<TcpStream>,
        value: &impl serde::Serialize,
    ) -> Result<()> {
        let response_json = serde_json::to_string(value)?;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            response_json.len(),
            response_json
        );
        stream.write_all(response.as_bytes()).await?;
        Ok(())
    }

    async fn handle_not_found(stream: &mut RustlsTlsStream<TcpStream>) -> Result<()> {
        let response = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n";
        stream.write_all(response.as_bytes()).await?;
        Ok(())
    }

    pub async fn stop(&self) {
        *self.running.lock().await = false;
    }
}

fn parse_content_length(headers: &str) -> Option<usize> {
    for line in headers.lines() {
        if line.to_ascii_lowercase().starts_with("content-length:") {
            return line.split(':').nth(1)?.trim().parse().ok();
        }
    }
    None
}

fn parse_filename_from_headers(headers: &str) -> Option<String> {
    for line in headers.lines() {
        let lower = line.to_ascii_lowercase();
        if lower.contains("content-disposition") && lower.contains("filename=") {
            if let Some(start) = line.find("filename=") {
                let rest = &line[start + 9..];
                let name = rest
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .split(';')
                    .next()
                    .unwrap_or("")
                    .to_string();
                if !name.is_empty() {
                    return Some(name);
                }
            }
        }
    }
    None
}
