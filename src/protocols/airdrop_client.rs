//! Apple AirDrop HTTPS client (/Discover, /Ask, /Upload) using binary plist.

use anyhow::{Context, Result};
use plist::{Dictionary, Value};
use std::io::Write;
use std::net::SocketAddr;
use std::path::Path;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_native_tls::{native_tls, TlsConnector};

const SENDER_MODEL: &str = "Windows,1";

pub struct AirDropClient;

impl AirDropClient {
    /// Probe whether a receiver responds to /Discover (OpenDrop-style).
    pub async fn probe_discover(target: SocketAddr) -> Result<Option<String>> {
        let body = encode_plist(&Dictionary::new())?;
        let response = post_on_new_connection(target, "/Discover", &body, "application/octet-stream").await?;
        let value: Value = plist::from_bytes(&response).context("parse discover response")?;
        Ok(value
            .as_dictionary()
            .and_then(|d| d.get("ReceiverComputerName"))
            .and_then(|v| v.as_string())
            .map(str::to_string))
    }

    /// Send a file to an Apple device using the AirDrop HTTPS protocol.
    pub async fn send_file(target: SocketAddr, file_path: &Path, sender_id: &str) -> Result<()> {
        let hostname = hostname::get()?.to_string_lossy().to_string();
        let file_name = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file")
            .to_string();
        let file_data = tokio::fs::read(file_path).await?;

        let mut discover = Dictionary::new();
        discover.insert(
            "SenderModelName".to_string(),
            Value::String(SENDER_MODEL.to_string()),
        );
        discover.insert(
            "SenderComputerName".to_string(),
            Value::String(hostname.clone()),
        );
        post_on_new_connection(target, "/Discover", &encode_plist(&discover)?, "application/octet-stream")
            .await?;

        let mut ask = Dictionary::new();
        ask.insert(
            "SenderModelName".to_string(),
            Value::String(SENDER_MODEL.to_string()),
        );
        ask.insert("SenderComputerName".to_string(), Value::String(hostname));
        ask.insert(
            "BundleID".to_string(),
            Value::String("com.apple.finder".to_string()),
        );
        ask.insert("SenderID".to_string(), Value::String(sender_id.to_string()));
        ask.insert("ConvertMediaFormats".to_string(), Value::Boolean(false));

        let mut file_entry = Dictionary::new();
        file_entry.insert("FileName".to_string(), Value::String(file_name.clone()));
        file_entry.insert(
            "FileType".to_string(),
            Value::String(guess_uti(file_path)),
        );
        file_entry.insert(
            "FileBomPath".to_string(),
            Value::String(format!("./{}", file_name)),
        );
        file_entry.insert("FileIsDirectory".to_string(), Value::Boolean(false));
        file_entry.insert("ConvertMediaFormats".to_string(), Value::Integer(0.into()));
        ask.insert(
            "Files".to_string(),
            Value::Array(vec![Value::Dictionary(file_entry)]),
        );

        post_on_new_connection(target, "/Ask", &encode_plist(&ask)?, "application/octet-stream").await?;

        let cpio_body = build_gzip_cpio(file_path, &file_data)?;
        post_upload_chunked(target, &cpio_body).await?;

        Ok(())
    }
}

async fn post_on_new_connection(
    target: SocketAddr,
    path: &str,
    body: &[u8],
    content_type: &str,
) -> Result<Vec<u8>> {
    let connector = tls_connector()?;
    let stream = TcpStream::connect(target).await?;
    let mut tls = connector.connect("AirDrop", stream).await?;
    post_raw(&mut tls, path, body, content_type).await
}

async fn post_upload_chunked(target: SocketAddr, body: &[u8]) -> Result<()> {
    let connector = tls_connector()?;
    let stream = TcpStream::connect(target).await?;
    let mut tls = connector.connect("AirDrop", stream).await?;

    let request = "POST /Upload HTTP/1.1\r\n\
                   Host: AirDrop\r\n\
                   Content-Type: application/x-cpio\r\n\
                   Transfer-Encoding: chunked\r\n\
                   Expect: 100-continue\r\n\
                   Connection: close\r\n\
                   User-Agent: AirDrop/1.0\r\n\
                   Accept: */*\r\n\
                   \r\n";
    tls.write_all(request.as_bytes()).await?;
    tls.flush().await?;

    let mut buf = [0u8; 256];
    let n = tls.read(&mut buf).await.unwrap_or(0);
    let continue_resp = String::from_utf8_lossy(&buf[..n]);
    if !continue_resp.contains("100 Continue") && !continue_resp.is_empty() {
        anyhow::bail!(
            "Upload not accepted: {}",
            continue_resp.lines().next().unwrap_or("unknown")
        );
    }

    for chunk in body.chunks(8192) {
        let header = format!("{:x}\r\n", chunk.len());
        tls.write_all(header.as_bytes()).await?;
        tls.write_all(chunk).await?;
        tls.write_all(b"\r\n").await?;
    }
    tls.write_all(b"0\r\n\r\n").await?;
    tls.flush().await?;

    read_http_response(&mut tls).await?;
    Ok(())
}

fn tls_connector() -> Result<TlsConnector> {
    let connector = native_tls::TlsConnector::builder()
        .danger_accept_invalid_certs(true)
        .build()?;
    Ok(TlsConnector::from(connector))
}

async fn post_raw(
    tls: &mut tokio_native_tls::TlsStream<TcpStream>,
    path: &str,
    body: &[u8],
    content_type: &str,
) -> Result<Vec<u8>> {
    let request = format!(
        "POST {} HTTP/1.1\r\n\
         Host: AirDrop\r\n\
         Content-Type: {}\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         User-Agent: AirDrop/1.0\r\n\
         Accept: */*\r\n\
         \r\n",
        path, content_type, body.len()
    );
    tls.write_all(request.as_bytes()).await?;
    tls.write_all(body).await?;
    tls.flush().await?;
    read_http_response(tls).await
}

fn encode_plist(map: &Dictionary) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    Value::Dictionary(map.clone())
        .to_writer_binary(&mut buf)
        .context("encode plist")?;
    Ok(buf)
}

fn guess_uti(path: &Path) -> String {
    let guessed = mime_guess::from_path(path).first_or_octet_stream();
    match guessed.essence_str() {
        "image/jpeg" => "public.jpeg".to_string(),
        "image/png" => "public.png".to_string(),
        "image/gif" => "com.compuserve.gif".to_string(),
        "image/heic" => "public.heic".to_string(),
        "video/mp4" => "public.mpeg-4".to_string(),
        "video/quicktime" => "com.apple.quicktime-movie".to_string(),
        "application/pdf" => "com.adobe.pdf".to_string(),
        "text/plain" => "public.plain-text".to_string(),
        _ => "public.content".to_string(),
    }
}

fn build_gzip_cpio(path: &Path, data: &[u8]) -> Result<Vec<u8>> {
    use flate2::write::GzEncoder;
    use flate2::Compression;

    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file");
    let store_path = format!("./{}", file_name);

    let mut plain = Vec::new();
    write_newc_entry(&mut plain, &store_path, data)?;

    let mut gz = GzEncoder::new(Vec::new(), Compression::default());
    gz.write_all(&plain)?;
    Ok(gz.finish()?)
}

fn write_newc_entry(out: &mut Vec<u8>, name: &str, data: &[u8]) -> Result<()> {
    let name_bytes = name.as_bytes();
    let namesize = name_bytes.len() + 1;
    let filesize = data.len();

    let mut header = [0u8; 110];
    header[..6].copy_from_slice(b"070701");
    write_hex_field(&mut header[54..64], filesize);
    write_hex_field(&mut header[94..108], namesize);
    out.extend_from_slice(&header);
    out.extend_from_slice(name_bytes);
    out.push(0);
    pad_to_4(out);
    out.extend_from_slice(data);
    pad_to_4(out);

    let mut trailer = [0u8; 110];
    trailer[..6].copy_from_slice(b"070701");
    write_hex_field(&mut trailer[94..108], 11);
    out.extend_from_slice(&trailer);
    out.extend_from_slice(b"TRAILER!!!");
    out.push(0);
    pad_to_4(out);
    Ok(())
}

fn write_hex_field(slot: &mut [u8], value: usize) {
    let s = format!("{:08x}", value);
    slot[..8].copy_from_slice(s.as_bytes());
}

fn pad_to_4(buf: &mut Vec<u8>) {
    while buf.len() % 4 != 0 {
        buf.push(0);
    }
}

async fn read_http_response(
    tls: &mut tokio_native_tls::TlsStream<TcpStream>,
) -> Result<Vec<u8>> {
    let mut buffer = Vec::new();
    let mut temp = [0u8; 4096];
    loop {
        let n = tls.read(&mut temp).await?;
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
        .context("HTTP response headers")?;
    let headers = String::from_utf8_lossy(&buffer[..header_end]);
    if !headers.contains("200 OK") && !headers.contains("100 Continue") {
        anyhow::bail!(
            "AirDrop request failed: {}",
            headers.lines().next().unwrap_or("unknown")
        );
    }

    let mut body = buffer[header_end + 4..].to_vec();
    if let Some(len) = parse_content_length(&headers) {
        while body.len() < len {
            let n = tls.read(&mut temp).await?;
            if n == 0 {
                break;
            }
            body.extend_from_slice(&temp[..n]);
        }
        body.truncate(len);
    }
    Ok(body)
}

fn parse_content_length(headers: &str) -> Option<usize> {
    for line in headers.lines() {
        if line.to_ascii_lowercase().starts_with("content-length:") {
            return line.split(':').nth(1)?.trim().parse().ok();
        }
    }
    None
}
