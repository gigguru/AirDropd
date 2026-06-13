//! Local "Web Drop" receiver.
//!
//! A tiny plain-HTTP server that lets any phone (iPhone or Android) send files
//! to this PC with nothing but its camera and built-in browser — no app, no
//! account, no internet. The flow is:
//!
//!   1. AirDropd shows a QR code that encodes `http://<lan-ip>:8771/`.
//!   2. The guest points their camera at it and taps the banner.
//!   3. Safari opens the upload page served from here, on the local network.
//!   4. They pick files and tap Send; bytes go straight to this PC over Wi-Fi.
//!
//! Files land in the same receive folder as AirDrop transfers and surface
//! through the same notification pipeline.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

use crate::config::{self, SharedConfig};

/// Default port for the Web Drop upload server.
pub const WEB_DROP_PORT: u16 = 8771;

/// Hard cap on a single upload request body (4 GiB) to bound memory/disk abuse.
const MAX_BODY: u64 = 4 * 1024 * 1024 * 1024;

pub struct WebDropServer {
    port: u16,
    config: SharedConfig,
    received_tx: tokio::sync::broadcast::Sender<PathBuf>,
    running: Arc<Mutex<bool>>,
    /// Set after the TCP listener binds successfully.
    listening: Arc<AtomicBool>,
}

impl WebDropServer {
    pub fn new(
        port: u16,
        config: SharedConfig,
        received_tx: tokio::sync::broadcast::Sender<PathBuf>,
    ) -> Self {
        Self {
            port,
            config,
            received_tx,
            running: Arc::new(Mutex::new(false)),
            listening: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn is_listening(&self) -> bool {
        self.listening.load(Ordering::SeqCst)
    }

    /// URL encoded in the QR code — uses the best LAN IPv4 for this machine.
    pub fn qr_url(&self) -> Result<String> {
        let ip = crate::network::util::best_lan_ipv4()?;
        Ok(format!("http://{}:{}/", ip, self.port))
    }

    pub async fn start(&self) -> Result<()> {
        let listener = bind_dual_stack(self.port).await?;
        self.listening.store(true, Ordering::SeqCst);
        info!("Web Drop server listening on port {}", self.port);
        crate::activity::log(
            crate::activity::Category::Server,
            format!(
                "Web Drop ready on port {} — phone URL: {}",
                self.port,
                self.qr_url().unwrap_or_else(|_| format!("http://<lan-ip>:{}/", self.port))
            ),
        );

        *self.running.lock().await = true;
        let running = self.running.clone();
        let config = self.config.clone();
        let received_tx = self.received_tx.clone();

        tokio::spawn(async move {
            while *running.lock().await {
                match listener.accept().await {
                    Ok((stream, addr)) => {
                        let config = config.clone();
                        let received_tx = received_tx.clone();
                        tokio::spawn(async move {
                            if let Err(e) =
                                handle_connection(stream, addr, config, received_tx).await
                            {
                                debug!("Web Drop connection from {} ended: {}", addr, e);
                            }
                        });
                    }
                    Err(e) => {
                        warn!("Web Drop accept error: {}", e);
                        break;
                    }
                }
            }
        });

        Ok(())
    }

    pub async fn stop(&self) {
        *self.running.lock().await = false;
        self.listening.store(false, Ordering::SeqCst);
    }
}

/// Bind IPv4 + IPv6 so phones reach us regardless of address family.
async fn bind_dual_stack(port: u16) -> Result<TcpListener> {
    use socket2::{Domain, Protocol, Socket, Type};
    let dual = || -> Result<TcpListener> {
        let socket = Socket::new(Domain::IPV6, Type::STREAM, Some(Protocol::TCP))?;
        socket.set_only_v6(false)?;
        socket.set_reuse_address(true)?;
        let addr: SocketAddr = format!("[::]:{}", port).parse()?;
        socket.bind(&addr.into())?;
        socket.listen(128)?;
        socket.set_nonblocking(true)?;
        Ok(TcpListener::from_std(socket.into())?)
    };
    match dual() {
        Ok(l) => Ok(l),
        Err(e) => {
            info!("Web Drop dual-stack bind failed ({}); IPv4-only", e);
            Ok(TcpListener::bind(("0.0.0.0", port)).await?)
        }
    }
}

struct RequestHead {
    method: String,
    path: String,
    headers: HashMap<String, String>,
}

async fn read_head<R: AsyncReadExt + Unpin>(
    reader: &mut R,
    carry: &mut Vec<u8>,
) -> Result<RequestHead> {
    // Accumulate until the blank line that ends the header block.
    loop {
        if let Some(pos) = find_subslice(carry, b"\r\n\r\n") {
            let head_bytes = carry[..pos].to_vec();
            let rest = carry[pos + 4..].to_vec();
            *carry = rest;
            return parse_head(&head_bytes);
        }
        let mut buf = [0u8; 8192];
        let n = reader.read(&mut buf).await?;
        if n == 0 {
            return Err(anyhow!("connection closed before headers complete"));
        }
        carry.extend_from_slice(&buf[..n]);
        if carry.len() > 64 * 1024 {
            return Err(anyhow!("request headers too large"));
        }
    }
}

fn parse_head(bytes: &[u8]) -> Result<RequestHead> {
    let text = String::from_utf8_lossy(bytes);
    let mut lines = text.split("\r\n");
    let request_line = lines.next().ok_or_else(|| anyhow!("empty request"))?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("/").to_string();
    let mut headers = HashMap::new();
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
        }
    }
    Ok(RequestHead {
        method,
        path,
        headers,
    })
}

async fn handle_connection(
    stream: TcpStream,
    addr: SocketAddr,
    config: SharedConfig,
    received_tx: tokio::sync::broadcast::Sender<PathBuf>,
) -> Result<()> {
    let _ = stream.set_nodelay(true);
    let mut reader = BufReader::new(stream);
    let mut carry: Vec<u8> = Vec::new();

    loop {
        let head = match read_head(&mut reader, &mut carry).await {
            Ok(h) => h,
            Err(_) => break,
        };
        let path = head.path.split('?').next().unwrap_or("/").to_string();

        match (head.method.as_str(), path.as_str()) {
            ("GET", "/") | ("GET", "/index.html") => {
                let name = config
                    .read()
                    .map(|c| c.broadcast_name.clone())
                    .unwrap_or_else(|_| "this PC".to_string());
                let body = upload_page(&name);
                write_response(
                    reader.get_mut(),
                    200,
                    "text/html; charset=utf-8",
                    body.as_bytes(),
                    true,
                )
                .await?;
                break;
            }
            ("HEAD", "/") | ("HEAD", "/index.html") => {
                let name = config
                    .read()
                    .map(|c| c.broadcast_name.clone())
                    .unwrap_or_else(|_| "this PC".to_string());
                let body = upload_page(&name);
                write_response_inner(
                    reader.get_mut(),
                    200,
                    "text/html; charset=utf-8",
                    body.as_bytes(),
                    true,
                    false,
                )
                .await?;
                break;
            }
            ("GET", "/ping") => {
                write_response(reader.get_mut(), 200, "text/plain", b"ok", true).await?;
                break;
            }
            ("GET", "/favicon.ico") => {
                write_response(reader.get_mut(), 204, "image/x-icon", b"", true).await?;
                break;
            }
            ("POST", "/upload") => {
                let saved =
                    receive_upload(&head, &mut reader, &mut carry, &config, &received_tx, addr)
                        .await;
                match saved {
                    Ok(n) => {
                        let msg = format!("{{\"ok\":true,\"saved\":{}}}", n);
                        write_response(
                            reader.get_mut(),
                            200,
                            "application/json",
                            msg.as_bytes(),
                            true,
                        )
                        .await?;
                    }
                    Err(e) => {
                        warn!("Web Drop upload failed: {}", e);
                        crate::activity::log(
                            crate::activity::Category::Error,
                            format!("Web Drop upload failed: {}", e),
                        );
                        write_response(
                            reader.get_mut(),
                            400,
                            "application/json",
                            b"{\"ok\":false}",
                            true,
                        )
                        .await?;
                    }
                }
                // Body is consumed; safest to close after an upload.
                break;
            }
            _ => {
                write_response(reader.get_mut(), 404, "text/plain", b"Not found", true).await?;
                break;
            }
        }
    }
    Ok(())
}

async fn write_response<W: AsyncWriteExt + Unpin>(
    w: &mut W,
    status: u16,
    content_type: &str,
    body: &[u8],
    close: bool,
) -> Result<()> {
    write_response_inner(w, status, content_type, body, close, true).await
}

async fn write_response_inner<W: AsyncWriteExt + Unpin>(
    w: &mut W,
    status: u16,
    content_type: &str,
    body: &[u8],
    close: bool,
    send_body: bool,
) -> Result<()> {
    let reason = match status {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        404 => "Not Found",
        _ => "OK",
    };
    let connection = if close { "close" } else { "keep-alive" };
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\n\
         Content-Type: {content_type}\r\n\
         Content-Length: {len}\r\n\
         Cache-Control: no-store\r\n\
         Connection: {connection}\r\n\r\n",
        len = body.len()
    );
    w.write_all(head.as_bytes()).await?;
    if send_body && !body.is_empty() {
        w.write_all(body).await?;
    }
    w.flush().await?;
    Ok(())
}

/// Parse a multipart/form-data POST and write each file part to the receive
/// folder, streaming to disk so large files don't balloon memory.
async fn receive_upload<R: AsyncReadExt + Unpin>(
    head: &RequestHead,
    reader: &mut R,
    carry: &mut Vec<u8>,
    config: &SharedConfig,
    received_tx: &tokio::sync::broadcast::Sender<PathBuf>,
    addr: SocketAddr,
) -> Result<usize> {
    let content_type = head
        .headers
        .get("content-type")
        .ok_or_else(|| anyhow!("missing content-type"))?;
    let boundary = content_type
        .split("boundary=")
        .nth(1)
        .map(|b| b.trim().trim_matches('"').to_string())
        .ok_or_else(|| anyhow!("missing multipart boundary"))?;
    let content_length: u64 = head
        .headers
        .get("content-length")
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| anyhow!("missing content-length"))?;
    if content_length > MAX_BODY {
        return Err(anyhow!("upload exceeds size limit"));
    }

    let receive_base = {
        let cfg = config.read().map_err(|_| anyhow!("config lock poisoned"))?;
        cfg.ensure_receive_dir()?
    };

    let client_ip = addr.ip().to_string();
    let mut guest_id = String::new();
    let mut guest_label = String::new();

    let delim = format!("--{}", boundary).into_bytes();
    let mut parser = MultipartReader::new(reader, carry, content_length, delim);
    let mut saved = 0usize;

    while let Some(part) = parser.next_part().await? {
        if let Some(filename) = part.filename.filter(|f| !f.is_empty()) {
            let device_dir = {
                let mut cfg = config.write().map_err(|_| anyhow!("config lock poisoned"))?;
                cfg.resolve_webdrop_folder(&guest_id, &guest_label, &client_ip)?
            };
            let folder_label = device_dir
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "WebDrop".to_string());
            let path = config::unique_receive_path(&device_dir, &filename);
            let mut file = tokio::fs::File::create(&path).await?;
            let written = parser.stream_part_to(&mut file).await?;
            file.flush().await?;
            info!(
                "Web Drop saved {} ({} bytes) from {} → {}",
                path.display(),
                written,
                client_ip,
                device_dir.display()
            );
            let folder = folder_label.as_str();
            crate::activity::log(
                crate::activity::Category::Transfer,
                format!(
                    "Web Drop received \"{}\" ({:.1} MB) → folder \"{}\"",
                    path.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default(),
                    written as f64 / 1_048_576.0,
                    folder
                ),
            );
            let _ = received_tx.send(path);
            saved += 1;
        } else if part.field_name.as_deref() == Some("device_id") {
            guest_id = read_field_string(&mut parser, 80).await?;
        } else if part.field_name.as_deref() == Some("device_label") {
            guest_label = read_field_string(&mut parser, 128).await?;
        } else {
            parser.skip_part().await?;
        }
    }

    let _ = receive_base;
    Ok(saved)
}

/// Incremental multipart/form-data reader that streams part bodies to a writer
/// without buffering the whole upload in memory.
struct MultipartReader<'a, R: AsyncReadExt + Unpin> {
    reader: &'a mut R,
    buf: Vec<u8>,
    remaining: u64,
    delim: Vec<u8>,
    started: bool,
    finished: bool,
}

struct PartHeader {
    field_name: Option<String>,
    filename: Option<String>,
}

impl<'a, R: AsyncReadExt + Unpin> MultipartReader<'a, R> {
    fn new(reader: &'a mut R, carry: &mut Vec<u8>, content_length: u64, delim: Vec<u8>) -> Self {
        let buf = std::mem::take(carry);
        let consumed = buf.len() as u64;
        Self {
            reader,
            buf,
            remaining: content_length.saturating_sub(consumed),
            delim,
            started: false,
            finished: false,
        }
    }

    async fn fill(&mut self, want: usize) -> Result<bool> {
        while self.buf.len() < want && self.remaining > 0 {
            let mut tmp = [0u8; 16384];
            let to_read = tmp.len().min(self.remaining as usize);
            let n = self.reader.read(&mut tmp[..to_read]).await?;
            if n == 0 {
                self.remaining = 0;
                break;
            }
            self.buf.extend_from_slice(&tmp[..n]);
            self.remaining -= n as u64;
        }
        Ok(self.buf.len() >= want)
    }

    async fn read_line(&mut self) -> Result<Option<Vec<u8>>> {
        loop {
            if let Some(pos) = find_subslice(&self.buf, b"\r\n") {
                let line = self.buf[..pos].to_vec();
                self.buf.drain(..pos + 2);
                return Ok(Some(line));
            }
            if !self.fill(self.buf.len() + 1).await? {
                return Ok(None);
            }
        }
    }

    /// Advance to the next part, returning its parsed headers, or None at end.
    async fn next_part(&mut self) -> Result<Option<PartHeader>> {
        if self.finished {
            return Ok(None);
        }
        if !self.started {
            // First boundary line.
            let line = self.read_line().await?.ok_or_else(|| anyhow!("no data"))?;
            if !starts_with(&line, &self.delim) {
                return Err(anyhow!("malformed multipart start"));
            }
            if is_closing(&line, &self.delim) {
                self.finished = true;
                return Ok(None);
            }
            self.started = true;
        }

        // Read part headers until blank line.
        let mut field_name = None;
        let mut filename = None;
        loop {
            let line = self.read_line().await?.ok_or_else(|| anyhow!("eof in headers"))?;
            if line.is_empty() {
                break;
            }
            let text = String::from_utf8_lossy(&line);
            if text.to_ascii_lowercase().contains("content-disposition") {
                let (name, file) = parse_content_disposition(&text);
                if field_name.is_none() {
                    field_name = name;
                }
                if file.is_some() {
                    filename = file;
                }
            }
        }
        Ok(Some(PartHeader {
            field_name,
            filename,
        }))
    }

    /// Stream the current part body to `out` until the next boundary.
    /// Returns bytes written. Also advances state to read the next part.
    async fn stream_part_to<W: AsyncWriteExt + Unpin>(&mut self, out: &mut W) -> Result<u64> {
        let needle = {
            let mut v = Vec::with_capacity(self.delim.len() + 2);
            v.extend_from_slice(b"\r\n");
            v.extend_from_slice(&self.delim);
            v
        };
        let mut written = 0u64;
        loop {
            if let Some(pos) = find_subslice(&self.buf, &needle) {
                // Everything before the boundary is body.
                out.write_all(&self.buf[..pos]).await?;
                written += pos as u64;
                // Consume body + leading CRLF of the delimiter.
                self.buf.drain(..pos + 2);
                self.consume_boundary().await?;
                return Ok(written);
            }
            // Keep a tail that might contain a partial boundary.
            let keep = needle.len();
            if self.buf.len() > keep {
                let flush_to = self.buf.len() - keep;
                out.write_all(&self.buf[..flush_to]).await?;
                written += flush_to as u64;
                self.buf.drain(..flush_to);
            }
            if !self.fill(self.buf.len() + 1).await? {
                // No more data; flush whatever's left.
                if !self.buf.is_empty() {
                    out.write_all(&self.buf).await?;
                    written += self.buf.len() as u64;
                    self.buf.clear();
                }
                self.finished = true;
                return Ok(written);
            }
        }
    }

    /// Drain the current part body (non-file field) up to the next boundary.
    async fn skip_part(&mut self) -> Result<()> {
        let mut sink = tokio::io::sink();
        self.stream_part_to(&mut sink).await?;
        Ok(())
    }

    /// After a body delimiter, read the rest of the delimiter line and decide
    /// whether the stream is finished (closing `--`) or another part follows.
    async fn consume_boundary(&mut self) -> Result<()> {
        let line = self.read_line().await?.unwrap_or_default();
        // `line` is the delimiter remainder, e.g. "" then boundary already
        // consumed via needle; detect closing marker.
        if line.len() >= 2 && &line[line.len() - 2..] == b"--" {
            self.finished = true;
        } else if is_closing(&line, &self.delim) {
            self.finished = true;
        }
        Ok(())
    }
}

async fn read_field_string<R: AsyncReadExt + Unpin>(
    parser: &mut MultipartReader<'_, R>,
    max: usize,
) -> Result<String> {
    let mut buf = Vec::new();
    let n = parser.stream_part_to(&mut buf).await? as usize;
    if n > max {
        return Err(anyhow!("form field exceeds {} bytes", max));
    }
    Ok(String::from_utf8_lossy(&buf).trim().to_string())
}

fn parse_content_disposition(text: &str) -> (Option<String>, Option<String>) {
    let lower = text.to_ascii_lowercase();
    let name = lower
        .find("name=")
        .and_then(|idx| extract_quoted_value(&text[idx + 5..]));
    let filename = extract_filename(text);
    (name, filename)
}

fn extract_quoted_value(rest: &str) -> Option<String> {
    let rest = rest.trim_start();
    if let Some(stripped) = rest.strip_prefix('"') {
        let end = stripped.find('"')?;
        Some(stripped[..end].to_string())
    } else {
        let end = rest.find([';', ' ', '\r', '\n']).unwrap_or(rest.len());
        let v = rest[..end].trim();
        if v.is_empty() {
            None
        } else {
            Some(v.to_string())
        }
    }
}

fn extract_filename(disposition: &str) -> Option<String> {
    // Look for filename="..."; supports the common Safari/Chrome form.
    let lower = disposition.to_ascii_lowercase();
    let idx = lower.find("filename=")?;
    let rest = &disposition[idx + "filename=".len()..];
    let rest = rest.trim_start();
    if let Some(stripped) = rest.strip_prefix('"') {
        let end = stripped.find('"')?;
        Some(sanitize(&stripped[..end]))
    } else {
        let end = rest.find([';', '\r', '\n']).unwrap_or(rest.len());
        Some(sanitize(rest[..end].trim()))
    }
}

fn sanitize(name: &str) -> String {
    name.rsplit(['/', '\\'])
        .next()
        .unwrap_or(name)
        .trim()
        .to_string()
}

fn starts_with(hay: &[u8], prefix: &[u8]) -> bool {
    hay.len() >= prefix.len() && &hay[..prefix.len()] == prefix
}

fn is_closing(line: &[u8], delim: &[u8]) -> bool {
    starts_with(line, delim) && line.len() >= delim.len() + 2 && &line[delim.len()..delim.len() + 2] == b"--"
}

fn find_subslice(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    hay.windows(needle.len()).position(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn collect_parts(body: &[u8], boundary: &str) -> Vec<(Option<String>, Vec<u8>)> {
        let mut carry: Vec<u8> = Vec::new();
        let mut reader: &[u8] = body;
        let delim = format!("--{}", boundary).into_bytes();
        let mut parser = MultipartReader::new(&mut reader, &mut carry, body.len() as u64, delim);
        let mut out = Vec::new();
        while let Some(part) = parser.next_part().await.unwrap() {
            let mut buf: Vec<u8> = Vec::new();
            parser.stream_part_to(&mut buf).await.unwrap();
            out.push((part.filename, buf));
        }
        out
    }

    #[tokio::test]
    async fn parses_multiple_file_parts() {
        let b = "X-BOUND-123";
        let binary = b"\x00\x01\x02BINARY\xfe\xfd";
        let mut body = format!(
            "--{b}\r\n\
             Content-Disposition: form-data; name=\"files\"; filename=\"hello.txt\"\r\n\
             Content-Type: text/plain\r\n\r\n\
             Hello, world!\r\n\
             --{b}\r\n\
             Content-Disposition: form-data; name=\"files\"; filename=\"track.bin\"\r\n\r\n",
            b = b
        )
        .into_bytes();
        body.extend_from_slice(binary);
        body.extend_from_slice(format!("\r\n--{b}--\r\n", b = b).as_bytes());
        let parts = collect_parts(&body, b).await;
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].0.as_deref(), Some("hello.txt"));
        assert_eq!(parts[0].1, b"Hello, world!");
        assert_eq!(parts[1].0.as_deref(), Some("track.bin"));
        assert_eq!(parts[1].1, binary);
    }

    #[tokio::test]
    async fn strips_path_from_filename() {
        let b = "B";
        let body = format!(
            "--{b}\r\n\
             Content-Disposition: form-data; name=\"f\"; filename=\"/var/evil/../song.mp3\"\r\n\r\n\
             DATA\r\n\
             --{b}--\r\n",
            b = b
        );
        let parts = collect_parts(body.as_bytes(), b).await;
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].0.as_deref(), Some("song.mp3"));
        assert_eq!(parts[0].1, b"DATA");
    }

    #[tokio::test]
    async fn reads_device_form_fields() {
        let b = "BOUND";
        let body = format!(
            "--{b}\r\n\
             Content-Disposition: form-data; name=\"device_id\"\r\n\r\n\
             abc-123-def\r\n\
             --{b}\r\n\
             Content-Disposition: form-data; name=\"device_label\"\r\n\r\n\
             Sarah's iPhone\r\n\
             --{b}\r\n\
             Content-Disposition: form-data; name=\"files\"; filename=\"track.mp3\"\r\n\r\n\
             DATA\r\n\
             --{b}--\r\n",
            b = b
        );
        let mut carry = Vec::new();
        let mut reader = body.as_bytes();
        let delim = format!("--{}", b).into_bytes();
        let mut parser =
            MultipartReader::new(&mut reader, &mut carry, body.len() as u64, delim);
        let mut guest_id = String::new();
        let mut guest_label = String::new();
        let mut files = Vec::new();
        while let Some(part) = parser.next_part().await.unwrap() {
            if let Some(filename) = part.filename.filter(|f| !f.is_empty()) {
                let mut buf = Vec::new();
                parser.stream_part_to(&mut buf).await.unwrap();
                files.push((filename, buf));
            } else if part.field_name.as_deref() == Some("device_id") {
                guest_id = read_field_string(&mut parser, 80).await.unwrap();
            } else if part.field_name.as_deref() == Some("device_label") {
                guest_label = read_field_string(&mut parser, 128).await.unwrap();
            } else {
                parser.skip_part().await.unwrap();
            }
        }
        assert_eq!(guest_id, "abc-123-def");
        assert_eq!(guest_label, "Sarah's iPhone");
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].0, "track.mp3");
    }
}

/// The mobile upload page. Fully self-contained — no external CSS/JS/fonts —
/// so it works with zero internet access.
fn upload_page(device_name: &str) -> String {
    let safe_name = device_name
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
<meta name="theme-color" content="#0a0a0c">
<title>Send to {name}</title>
<style>
  :root {{ color-scheme: dark; }}
  * {{ box-sizing: border-box; -webkit-tap-highlight-color: transparent; }}
  body {{
    margin: 0; min-height: 100vh; padding: env(safe-area-inset-top) 20px 40px;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
    background: radial-gradient(120% 120% at 50% 0%, #1c1c22 0%, #0a0a0c 60%);
    color: #f2f2f7; display: flex; flex-direction: column; align-items: center;
  }}
  .logo {{ font-size: 44px; margin: 34px 0 6px; }}
  h1 {{ font-size: 22px; font-weight: 600; margin: 0 0 4px; text-align: center; }}
  .sub {{ color: #9b9ba3; font-size: 14px; margin: 0 0 26px; text-align: center; }}
  .card {{
    width: 100%; max-width: 460px; background: rgba(255,255,255,0.06);
    border: 1px solid rgba(255,255,255,0.08); border-radius: 22px; padding: 22px;
    backdrop-filter: blur(20px);
  }}
  .drop {{
    border: 2px dashed rgba(255,255,255,0.22); border-radius: 16px; padding: 34px 18px;
    text-align: center; color: #c7c7cf; font-size: 15px; transition: 0.2s;
  }}
  .drop.active {{ border-color: #0a84ff; background: rgba(10,132,255,0.08); }}
  input[type=file] {{ display: none; }}
  .field {{ width: 100%; margin-bottom: 14px; }}
  .field label {{ display: block; font-size: 13px; color: #9b9ba3; margin-bottom: 6px; }}
  .field input[type=text] {{
    width: 100%; padding: 12px 14px; border-radius: 12px; border: 1px solid rgba(255,255,255,0.12);
    background: rgba(0,0,0,0.25); color: #f2f2f7; font-size: 16px;
  }}
  .btn {{
    display: block; width: 100%; margin-top: 16px; padding: 16px; border: none;
    border-radius: 14px; background: #0a84ff; color: #fff; font-size: 17px;
    font-weight: 600; cursor: pointer; transition: 0.15s;
  }}
  .btn:active {{ transform: scale(0.985); }}
  .btn.secondary {{ background: rgba(255,255,255,0.1); }}
  .btn:disabled {{ opacity: 0.5; }}
  ul {{ list-style: none; padding: 0; margin: 16px 0 0; }}
  li {{
    display: flex; justify-content: space-between; gap: 10px; padding: 10px 12px;
    background: rgba(255,255,255,0.05); border-radius: 10px; margin-bottom: 8px;
    font-size: 14px;
  }}
  li .sz {{ color: #9b9ba3; flex: none; }}
  .bar {{ height: 8px; border-radius: 4px; background: rgba(255,255,255,0.1); overflow: hidden; margin-top: 16px; display: none; }}
  .bar > i {{ display: block; height: 100%; width: 0; background: #30d158; transition: width 0.2s; }}
  .status {{ text-align: center; margin-top: 14px; font-size: 15px; min-height: 20px; }}
  .ok {{ color: #30d158; }} .err {{ color: #ff453a; }}
</style>
</head>
<body>
  <div class="logo">📡</div>
  <h1>Send to {name}</h1>
  <p class="sub">Pick files and tap Send — saved in your folder on this computer. Same phone = same folder every time.</p>
  <div class="card">
    <div class="field">
      <label for="label">Your name (for the DJ folder)</label>
      <input type="text" id="label" placeholder="e.g. Sarah's iPhone" autocomplete="name">
    </div>
    <label class="drop" id="drop">
      <input type="file" id="file" multiple>
      <div id="dropText">Tap to choose photos, videos or files</div>
    </label>
    <ul id="list"></ul>
    <div class="bar" id="bar"><i id="fill"></i></div>
    <div class="status" id="status"></div>
    <button class="btn" id="send" disabled>Send</button>
    <button class="btn secondary" id="more" style="display:none">Send more</button>
  </div>
<script>
  var fileInput = document.getElementById('file');
  var drop = document.getElementById('drop');
  var list = document.getElementById('list');
  var sendBtn = document.getElementById('send');
  var moreBtn = document.getElementById('more');
  var status = document.getElementById('status');
  var bar = document.getElementById('bar');
  var fill = document.getElementById('fill');
  var chosen = [];
  var DEVICE_KEY = 'airdropd-device-id';
  var LABEL_KEY = 'airdropd-device-label';
  function makeId() {{
    if (window.crypto && crypto.randomUUID) return crypto.randomUUID();
    return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, function(c) {{
      var r = Math.random() * 16 | 0, v = c === 'x' ? r : (r & 0x3 | 0x8);
      return v.toString(16);
    }});
  }}
  var deviceId = localStorage.getItem(DEVICE_KEY);
  if (!deviceId) {{ deviceId = makeId(); localStorage.setItem(DEVICE_KEY, deviceId); }}
  var labelInput = document.getElementById('label');
  labelInput.value = localStorage.getItem(LABEL_KEY) || '';
  labelInput.addEventListener('input', function() {{ localStorage.setItem(LABEL_KEY, labelInput.value); }});

  function fmt(n) {{
    if (n < 1024) return n + ' B';
    if (n < 1048576) return (n/1024).toFixed(0) + ' KB';
    if (n < 1073741824) return (n/1048576).toFixed(1) + ' MB';
    return (n/1073741824).toFixed(2) + ' GB';
  }}
  function render() {{
    list.innerHTML = '';
    chosen.forEach(function(f) {{
      var li = document.createElement('li');
      var nm = document.createElement('span'); nm.textContent = f.name;
      var sz = document.createElement('span'); sz.className = 'sz'; sz.textContent = fmt(f.size);
      li.appendChild(nm); li.appendChild(sz); list.appendChild(li);
    }});
    sendBtn.disabled = chosen.length === 0;
  }}
  fileInput.addEventListener('change', function() {{
    chosen = Array.prototype.slice.call(fileInput.files);
    document.getElementById('dropText').textContent =
      chosen.length ? (chosen.length + ' file' + (chosen.length>1?'s':'') + ' selected') : 'Tap to choose photos, videos or files';
    status.textContent = ''; render();
  }});
  ['dragover','dragenter'].forEach(function(ev){{ drop.addEventListener(ev, function(e){{ e.preventDefault(); drop.classList.add('active'); }}); }});
  ['dragleave','drop'].forEach(function(ev){{ drop.addEventListener(ev, function(e){{ e.preventDefault(); drop.classList.remove('active'); }}); }});
  drop.addEventListener('drop', function(e){{ if (e.dataTransfer && e.dataTransfer.files.length) {{ chosen = Array.prototype.slice.call(e.dataTransfer.files); render(); }} }});

  sendBtn.addEventListener('click', function() {{
    if (!chosen.length) return;
    var fd = new FormData();
    fd.append('device_id', deviceId);
    fd.append('device_label', labelInput.value.trim());
    chosen.forEach(function(f) {{ fd.append('files', f, f.name); }});
    var xhr = new XMLHttpRequest();
    xhr.open('POST', '/upload');
    sendBtn.disabled = true; sendBtn.textContent = 'Sending…';
    bar.style.display = 'block'; status.textContent = ''; status.className = 'status';
    xhr.upload.onprogress = function(e) {{ if (e.lengthComputable) fill.style.width = (e.loaded/e.total*100) + '%'; }};
    xhr.onload = function() {{
      if (xhr.status === 200) {{
        fill.style.width = '100%';
        status.textContent = '✓ Sent to {name}'; status.className = 'status ok';
        sendBtn.style.display = 'none'; moreBtn.style.display = 'block';
      }} else {{ fail(); }}
    }};
    xhr.onerror = fail;
    xhr.send(fd);
    function fail() {{
      status.textContent = 'Upload failed — check you are on the same Wi-Fi and try again.';
      status.className = 'status err';
      sendBtn.disabled = false; sendBtn.textContent = 'Send'; bar.style.display = 'none';
    }}
  }});
  moreBtn.addEventListener('click', function() {{
    chosen = []; fileInput.value = ''; render();
    document.getElementById('dropText').textContent = 'Tap to choose photos, videos or files';
    moreBtn.style.display = 'none'; sendBtn.style.display = 'block';
    sendBtn.textContent = 'Send'; bar.style.display = 'none'; fill.style.width = '0';
    status.textContent = '';
  }});
</script>
</body>
</html>"##,
        name = safe_name
    )
}
