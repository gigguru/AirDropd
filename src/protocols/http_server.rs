use anyhow::{anyhow, Context, Result};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio_rustls::server::TlsStream as RustlsTlsStream;
use tokio_rustls::TlsAcceptor;
use tracing::{debug, error, info};

use crate::config::{self, SharedConfig};
use super::apple_plist;
use super::tls_store;

const RECEIVER_MODEL: &str = "Windows,1";

/// HTTP/HTTPS server implementing Apple AirDrop binary-plist protocol.
pub struct AirDropHttpServer {
    port: u16,
    config: SharedConfig,
    received_tx: Option<tokio::sync::broadcast::Sender<PathBuf>>,
    tls_acceptor: Option<TlsAcceptor>,
    running: Arc<Mutex<bool>>,
}

struct HttpRequest {
    method: String,
    path: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
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

    pub async fn initialize(&mut self) -> Result<()> {
        let computer_name = self
            .config
            .read()
            .map(|c| c.broadcast_name.clone())
            .unwrap_or_else(|_| config::default_broadcast_name());
        let tls_config = tls_store::load_or_create_server_config(&computer_name)?;
        self.tls_acceptor = Some(TlsAcceptor::from(tls_config));
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

        loop {
            let request = match read_http_request(&mut tls_stream).await {
                Ok(req) => req,
                Err(e) => {
                    debug!("Connection from {} ended: {}", addr, e);
                    break;
                }
            };

            debug!("HTTP {} {}", request.method, request.path);

            let close_connection = match (request.method.as_str(), request.path.as_str()) {
                ("GET", "/") | ("HEAD", "/") => {
                    write_response(&mut tls_stream, 200, &[], false).await?;
                    false
                }
                ("POST", "/Discover") => {
                    handle_discover(&mut tls_stream, &request, &config).await?;
                    false
                }
                ("POST", "/Ask") => {
                    handle_ask(&mut tls_stream, &request, &config).await?;
                    false
                }
                ("POST", "/Upload") => {
                    handle_upload(&mut tls_stream, &request, &config, received_tx.clone()).await?;
                    true
                }
                _ => {
                    write_response(&mut tls_stream, 404, &[], true).await?;
                    true
                }
            };

            if close_connection {
                break;
            }
        }

        Ok(())
    }

    pub async fn stop(&self) {
        *self.running.lock().await = false;
    }
}

async fn handle_discover(
    stream: &mut RustlsTlsStream<TcpStream>,
    request: &HttpRequest,
    config: &SharedConfig,
) -> Result<()> {
    if !request.body.is_empty() {
        if let Ok(plist) = apple_plist::parse_plist(&request.body) {
            let sender = apple_plist::plist_string(&plist, "SenderComputerName")
                .or_else(|| apple_plist::plist_string(&plist, "SenderModelName"));
            if let Some(name) = sender {
                info!("AirDrop /Discover from {}", name);
            }
        }
    } else {
        info!("AirDrop /Discover (empty body)");
    }

    let (broadcast_name, flags) = {
        let cfg = config
            .read()
            .map_err(|_| anyhow!("config lock poisoned"))?;
        (
            cfg.broadcast_name.clone(),
            apple_plist::receiver_flags(cfg.discoverable, cfg.contacts_only),
        )
    };

    let body = apple_plist::build_discover_response(&broadcast_name, RECEIVER_MODEL, flags)?;
    write_response(stream, 200, &body, false).await
}

async fn handle_ask(
    stream: &mut RustlsTlsStream<TcpStream>,
    request: &HttpRequest,
    config: &SharedConfig,
) -> Result<()> {
    if let Ok(plist) = apple_plist::parse_plist(&request.body) {
        let sender = apple_plist::plist_string(&plist, "SenderComputerName")
            .unwrap_or_else(|| "Unknown".to_string());
        info!("AirDrop /Ask from {} — accepting transfer", sender);
    }

    let broadcast_name = config
        .read()
        .map(|c| c.broadcast_name.clone())
        .unwrap_or_else(|_| config::default_broadcast_name());

    let body = apple_plist::build_ask_response(&broadcast_name, RECEIVER_MODEL)?;
    write_response(stream, 200, &body, false).await
}

async fn handle_upload(
    stream: &mut RustlsTlsStream<TcpStream>,
    request: &HttpRequest,
    config: &SharedConfig,
    received_tx: Option<tokio::sync::broadcast::Sender<PathBuf>>,
) -> Result<()> {
    info!("AirDrop /Upload from Apple device");

    let content_type = header_value(&request.headers, "content-type");
    if !content_type.is_empty() && !content_type.contains("application/x-cpio") {
        write_response(stream, 406, &[], true).await?;
        return Ok(());
    }

    if header_value(&request.headers, "expect").eq_ignore_ascii_case("100-continue") {
        stream.write_all(b"HTTP/1.1 100 Continue\r\n\r\n").await?;
    }

    let receive_dir = {
        let cfg = config.read().map_err(|_| anyhow!("config lock poisoned"))?;
        cfg.ensure_receive_dir()?
    };

    let transfer_encoding = header_value(&request.headers, "transfer-encoding");
    let payload = if transfer_encoding.contains("chunked") {
        read_chunked_body(stream, &request.body).await?
    } else if let Some(len) = content_length(&request.headers) {
        read_fixed_body(stream, &request.body, len).await?
    } else {
        request.body.clone()
    };

    let saved = cpio_extract::extract_gzip_cpio(std::io::Cursor::new(payload), &receive_dir)?;

    for path in &saved {
        info!("Saved received file to {:?}", path);
        if let Some(tx) = &received_tx {
            let _ = tx.send(path.clone());
        }
    }

    write_response(stream, 200, &[], true).await
}

async fn read_fixed_body(
    stream: &mut RustlsTlsStream<TcpStream>,
    initial: &[u8],
    len: usize,
) -> Result<Vec<u8>> {
    let mut body = initial.to_vec();
    while body.len() < len {
        let mut chunk = vec![0u8; 8192];
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..n]);
    }
    body.truncate(len);
    Ok(body)
}

async fn read_chunked_body(
    stream: &mut RustlsTlsStream<TcpStream>,
    initial: &[u8],
) -> Result<Vec<u8>> {
    let mut reader = ChunkedStreamReader::new(stream, initial.to_vec());
    let mut out = Vec::new();
    loop {
        let chunk = reader.next_chunk().await?;
        if chunk.is_empty() {
            break;
        }
        out.extend_from_slice(&chunk);
    }
    Ok(out)
}

struct ChunkedStreamReader<'a> {
    stream: &'a mut RustlsTlsStream<TcpStream>,
    pending: Vec<u8>,
    done: bool,
}

impl<'a> ChunkedStreamReader<'a> {
    fn new(stream: &'a mut RustlsTlsStream<TcpStream>, pending: Vec<u8>) -> Self {
        Self {
            stream,
            pending,
            done: false,
        }
    }

    async fn read_byte(&mut self) -> Result<Option<u8>> {
        if !self.pending.is_empty() {
            return Ok(Some(self.pending.remove(0)));
        }
        let mut b = [0u8; 1];
        let n = self.stream.read(&mut b).await?;
        if n == 0 {
            return Ok(None);
        }
        Ok(Some(b[0]))
    }

    async fn read_line(&mut self) -> Result<String> {
        let mut line = Vec::new();
        while let Some(b) = self.read_byte().await? {
            if b == b'\n' {
                break;
            }
            if b != b'\r' {
                line.push(b);
            }
        }
        Ok(String::from_utf8_lossy(&line).into_owned())
    }

    async fn read_exact(&mut self, buf: &mut [u8]) -> Result<()> {
        let mut filled = 0;
        while filled < buf.len() {
            if !self.pending.is_empty() {
                let take = (buf.len() - filled).min(self.pending.len());
                buf[filled..filled + take].copy_from_slice(&self.pending[..take]);
                self.pending.drain(..take);
                filled += take;
                continue;
            }
            let n = self.stream.read(&mut buf[filled..]).await?;
            if n == 0 {
                return Err(anyhow!("unexpected EOF reading chunked body"));
            }
            filled += n;
        }
        Ok(())
    }

    async fn next_chunk(&mut self) -> Result<Vec<u8>> {
        if self.done {
            return Ok(Vec::new());
        }

        let size_line = self.read_line().await?;
        let size = usize::from_str_radix(size_line.trim(), 16).context("chunk size")?;
        if size == 0 {
            self.done = true;
            let mut trailer = [0u8; 2];
            let _ = self.read_exact(&mut trailer).await;
            return Ok(Vec::new());
        }

        let mut data = vec![0u8; size];
        self.read_exact(&mut data).await?;
        let mut crlf = [0u8; 2];
        self.read_exact(&mut crlf).await?;
        Ok(data)
    }
}

async fn read_http_request(stream: &mut RustlsTlsStream<TcpStream>) -> Result<HttpRequest> {
    let mut buffer = Vec::new();
    let mut temp = [0u8; 8192];

    loop {
        let n = stream.read(&mut temp).await?;
        if n == 0 {
            break;
        }
        buffer.extend_from_slice(&temp[..n]);
        if buffer.windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
    }

    let header_end = buffer
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or_else(|| anyhow!("missing HTTP header terminator"))?;

    let header_text = String::from_utf8_lossy(&buffer[..header_end]);
    let mut lines = header_text.lines();
    let request_line = lines.next().ok_or_else(|| anyhow!("empty HTTP request"))?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("/").to_string();

    let mut headers = HashMap::new();
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
        }
    }

    let mut body = buffer[header_end + 4..].to_vec();
    if let Some(len) = content_length(&headers) {
        while body.len() < len {
            let mut chunk = vec![0u8; 8192];
            let n = stream.read(&mut chunk).await?;
            if n == 0 {
                break;
            }
            body.extend_from_slice(&chunk[..n]);
        }
        body.truncate(len);
    }

    Ok(HttpRequest {
        method,
        path,
        headers,
        body,
    })
}

async fn write_response(
    stream: &mut RustlsTlsStream<TcpStream>,
    status: u16,
    body: &[u8],
    close: bool,
) -> Result<()> {
    let reason = match status {
        200 => "OK",
        404 => "Not Found",
        406 => "Not Acceptable",
        _ => "Error",
    };
    let connection = if close { "close" } else { "keep-alive" };
    let response = format!(
        "HTTP/1.1 {} {}\r\nContent-Length: {}\r\nConnection: {}\r\n\r\n",
        status,
        reason,
        body.len(),
        connection
    );
    stream.write_all(response.as_bytes()).await?;
    if !body.is_empty() {
        stream.write_all(body).await?;
    }
    stream.flush().await?;
    Ok(())
}

fn header_value(headers: &HashMap<String, String>, key: &str) -> String {
    headers.get(key).cloned().unwrap_or_default()
}

fn content_length(headers: &HashMap<String, String>) -> Option<usize> {
    header_value(headers, "content-length").parse().ok()
}

use super::cpio_extract;
