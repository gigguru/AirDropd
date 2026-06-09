//! Apple AirDrop HTTPS client (/Discover, /Ask, /Upload).

use anyhow::{Context, Result};
use std::net::SocketAddr;
use std::path::Path;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_native_tls::{native_tls, TlsConnector};

pub struct AirDropClient;

impl AirDropClient {
    /// Send a file to an Apple device using the AirDrop HTTPS protocol.
    pub async fn send_file(target: SocketAddr, file_path: &Path) -> Result<()> {
        let hostname = hostname::get()?.to_string_lossy().to_string();
        let file_name = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file")
            .to_string();
        let file_data = tokio::fs::read(file_path).await?;
        let mime = mime_guess::from_path(file_path)
            .first_or_octet_stream()
            .essence_str()
            .to_string();

        let connector = native_tls::TlsConnector::builder()
            .danger_accept_invalid_certs(true)
            .build()?;
        let connector = TlsConnector::from(connector);

        let stream = TcpStream::connect(target).await?;
        let mut tls = connector.connect("AirDrop", stream).await?;

        // /Discover
        let discover_body = serde_json::json!({
            "SenderModelName": "Windows,1",
            "SenderComputerName": hostname,
        });
        Self::post_json(&mut tls, "/Discover", &discover_body).await?;

        // /Ask
        let ask_body = serde_json::json!({
            "SenderModelName": "Windows,1",
            "SenderComputerName": hostname,
            "Files": [{
                "FileName": file_name,
                "FileType": mime,
                "FileBomPath": file_name,
                "FileSize": file_data.len(),
            }]
        });
        Self::post_json(&mut tls, "/Ask", &ask_body).await?;

        // /Upload (raw file bytes after headers)
        let upload_headers = format!(
            "POST /Upload HTTP/1.1\r\n\
             Host: AirDrop\r\n\
             Content-Type: application/octet-stream\r\n\
             Content-Length: {}\r\n\
             \r\n",
            file_data.len()
        );
        tls.write_all(upload_headers.as_bytes()).await?;
        tls.write_all(&file_data).await?;
        tls.flush().await?;

        let mut resp = vec![0u8; 512];
        let n = tls.read(&mut resp).await.unwrap_or(0);
        tracing::info!(
            "AirDrop upload response: {}",
            String::from_utf8_lossy(&resp[..n])
        );
        Ok(())
    }

    async fn post_json<S: serde::Serialize>(
        tls: &mut tokio_native_tls::TlsStream<TcpStream>,
        path: &str,
        body: &S,
    ) -> Result<()> {
        let json = serde_json::to_string(body)?;
        let request = format!(
            "POST {} HTTP/1.1\r\n\
             Host: AirDrop\r\n\
             Content-Type: application/json\r\n\
             Content-Length: {}\r\n\
             \r\n\
             {}",
            path,
            json.len(),
            json
        );
        tls.write_all(request.as_bytes()).await?;
        tls.flush().await?;

        let mut buf = vec![0u8; 4096];
        let n = tls.read(&mut buf).await.context("read response")?;
        let response = String::from_utf8_lossy(&buf[..n]);
        if !response.contains("200 OK") {
            anyhow::bail!("AirDrop {} failed: {}", path, response.lines().next().unwrap_or(""));
        }
        Ok(())
    }
}
