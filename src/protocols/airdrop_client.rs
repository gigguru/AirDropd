//! Apple AirDrop HTTPS client (/Discover, /Ask, /Upload) using binary plist.
//!
//! Supports multi-file and folder transfers with streamed archive upload and
//! live progress reporting.

use anyhow::{anyhow, Context, Result};
use plist::{Dictionary, Value};
use std::io::Write;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_native_tls::{native_tls, TlsConnector};

const SENDER_MODEL: &str = "Windows,1";
const UPLOAD_CHUNK: usize = 65536;

/// Shared send-progress slot: `Some(percent 0..=100)` while a transfer runs.
pub type SendProgress = Arc<StdMutex<Option<f32>>>;

/// A single file resolved for sending (folders are expanded recursively).
struct SendEntry {
    abs: PathBuf,
    /// Relative path inside the archive, e.g. `My Set/track01.wav`
    rel: String,
    size: u64,
}

pub struct AirDropClient;

impl AirDropClient {
    /// Probe whether a receiver responds to /Discover (OpenDrop-style).
    pub async fn probe_discover(target: SocketAddr) -> Result<Option<String>> {
        let body = encode_plist(&Dictionary::new())?;
        let response =
            post_on_new_connection(target, "/Discover", &body, "application/octet-stream").await?;
        let value: Value = plist::from_bytes(&response).context("parse discover response")?;
        Ok(value
            .as_dictionary()
            .and_then(|d| d.get("ReceiverComputerName"))
            .and_then(|v| v.as_string())
            .map(str::to_string))
    }

    /// Send one file (kept for compatibility; forwards to `send_files`).
    pub async fn send_file(target: SocketAddr, file_path: &Path, sender_id: &str) -> Result<()> {
        Self::send_files(
            target,
            vec![file_path.to_path_buf()],
            sender_id,
            Arc::new(StdMutex::new(None)),
        )
        .await
    }

    /// Send any mix of files and folders to an AirDrop receiver.
    ///
    /// Folders are expanded recursively and their structure is preserved in
    /// the cpio archive. `progress` is updated with 0..=100 as bytes go out.
    pub async fn send_files(
        target: SocketAddr,
        paths: Vec<PathBuf>,
        sender_id: &str,
        progress: SendProgress,
    ) -> Result<()> {
        let result = Self::send_files_inner(target, paths, sender_id, &progress).await;
        if let Ok(mut slot) = progress.lock() {
            *slot = None;
        }
        result
    }

    async fn send_files_inner(
        target: SocketAddr,
        paths: Vec<PathBuf>,
        sender_id: &str,
        progress: &SendProgress,
    ) -> Result<()> {
        let entries = tokio::task::spawn_blocking(move || collect_entries(&paths))
            .await
            .context("file scan task")??;
        if entries.is_empty() {
            anyhow::bail!("Nothing to send: no readable files found");
        }

        let hostname = hostname::get()?.to_string_lossy().to_string();

        set_progress(progress, 0.0);

        // 1. /Discover — wake the receiver and confirm it speaks AirDrop.
        let mut discover = Dictionary::new();
        discover.insert(
            "SenderModelName".to_string(),
            Value::String(SENDER_MODEL.to_string()),
        );
        discover.insert(
            "SenderComputerName".to_string(),
            Value::String(hostname.clone()),
        );
        post_on_new_connection(
            target,
            "/Discover",
            &encode_plist(&discover)?,
            "application/octet-stream",
        )
        .await?;

        // 2. /Ask — request consent, listing every file.
        let ask = build_ask_plist(&hostname, sender_id, &entries, None)?;
        post_on_new_connection(target, "/Ask", &ask, "application/octet-stream")
            .await
            .context("receiver declined the transfer")?;

        // 3. Build the gzip cpio archive on disk (no large memory spikes).
        let archive_path = std::env::temp_dir().join(format!(
            "AirDropd-send-{}.cpio.gz",
            uuid::Uuid::new_v4()
        ));
        let archive_for_task = archive_path.clone();
        let build_result = tokio::task::spawn_blocking(move || {
            build_archive(&entries, &archive_for_task)
        })
        .await
        .context("archive task")?;
        if let Err(e) = build_result {
            let _ = std::fs::remove_file(&archive_path);
            return Err(e);
        }

        // 4. /Upload — stream the archive with chunked transfer encoding.
        let upload_result = post_upload_streamed(target, &archive_path, progress).await;
        let _ = tokio::fs::remove_file(&archive_path).await;
        upload_result?;

        set_progress(progress, 100.0);
        Ok(())
    }

    /// Share a URL via AirDrop (`Items` array in the /Ask request, no upload).
    pub async fn send_link(target: SocketAddr, url: &str, sender_id: &str) -> Result<()> {
        let hostname = hostname::get()?.to_string_lossy().to_string();

        let mut discover = Dictionary::new();
        discover.insert(
            "SenderModelName".to_string(),
            Value::String(SENDER_MODEL.to_string()),
        );
        discover.insert(
            "SenderComputerName".to_string(),
            Value::String(hostname.clone()),
        );
        post_on_new_connection(
            target,
            "/Discover",
            &encode_plist(&discover)?,
            "application/octet-stream",
        )
        .await?;

        let ask = build_ask_plist(&hostname, sender_id, &[], Some(url))?;
        post_on_new_connection(target, "/Ask", &ask, "application/octet-stream")
            .await
            .context("receiver declined the link")?;
        Ok(())
    }
}

fn set_progress(progress: &SendProgress, value: f32) {
    if let Ok(mut slot) = progress.lock() {
        *slot = Some(value);
    }
}

fn build_ask_plist(
    hostname: &str,
    sender_id: &str,
    entries: &[SendEntry],
    link: Option<&str>,
) -> Result<Vec<u8>> {
    let mut ask = Dictionary::new();
    ask.insert(
        "SenderModelName".to_string(),
        Value::String(SENDER_MODEL.to_string()),
    );
    ask.insert(
        "SenderComputerName".to_string(),
        Value::String(hostname.to_string()),
    );
    ask.insert(
        "BundleID".to_string(),
        Value::String("com.apple.finder".to_string()),
    );
    ask.insert("SenderID".to_string(), Value::String(sender_id.to_string()));
    ask.insert("ConvertMediaFormats".to_string(), Value::Boolean(false));

    if let Some(url) = link {
        ask.insert(
            "Items".to_string(),
            Value::Array(vec![Value::String(url.to_string())]),
        );
    }

    if !entries.is_empty() {
        let files: Vec<Value> = entries
            .iter()
            .map(|entry| {
                let file_name = Path::new(&entry.rel)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("file")
                    .to_string();
                let mut file_entry = Dictionary::new();
                file_entry.insert("FileName".to_string(), Value::String(file_name));
                file_entry.insert(
                    "FileType".to_string(),
                    Value::String(guess_uti(&entry.abs)),
                );
                file_entry.insert(
                    "FileBomPath".to_string(),
                    Value::String(format!("./{}", entry.rel)),
                );
                file_entry.insert("FileIsDirectory".to_string(), Value::Boolean(false));
                file_entry.insert("ConvertMediaFormats".to_string(), Value::Integer(0.into()));
                Value::Dictionary(file_entry)
            })
            .collect();
        ask.insert("Files".to_string(), Value::Array(files));
    }

    encode_plist(&ask)
}

/// Expand files and folders into archive entries with relative paths.
fn collect_entries(paths: &[PathBuf]) -> Result<Vec<SendEntry>> {
    let mut out = Vec::new();
    for path in paths {
        let meta = std::fs::metadata(path)
            .with_context(|| format!("cannot read {}", path.display()))?;
        if meta.is_dir() {
            let base = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("folder")
                .to_string();
            walk_dir(path, &base, &mut out)?;
        } else if meta.is_file() {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("file")
                .to_string();
            out.push(SendEntry {
                abs: path.clone(),
                rel: name,
                size: meta.len(),
            });
        }
    }
    Ok(out)
}

fn walk_dir(dir: &Path, rel_base: &str, out: &mut Vec<SendEntry>) -> Result<()> {
    let entries = std::fs::read_dir(dir)
        .with_context(|| format!("cannot read folder {}", dir.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let rel = format!("{}/{}", rel_base, name);
        let Ok(meta) = entry.metadata() else { continue };
        if meta.is_dir() {
            walk_dir(&path, &rel, out)?;
        } else if meta.is_file() {
            out.push(SendEntry {
                abs: path,
                rel,
                size: meta.len(),
            });
        }
    }
    Ok(())
}

/// Write all entries as a gzip-compressed newc cpio archive, streaming file
/// contents so memory stays flat regardless of transfer size.
fn build_archive(entries: &[SendEntry], out_path: &Path) -> Result<()> {
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::collections::HashSet;
    use std::io::{BufWriter, Read};

    let file = std::fs::File::create(out_path)?;
    let mut gz = GzEncoder::new(BufWriter::new(file), Compression::default());

    // Directory records first so extractors recreate the tree.
    let mut dirs_written: HashSet<String> = HashSet::new();
    for entry in entries {
        let mut ancestors = Vec::new();
        let mut current = Path::new(&entry.rel).parent();
        while let Some(parent) = current {
            let p = parent.to_string_lossy().replace('\\', "/");
            if p.is_empty() {
                break;
            }
            ancestors.push(p);
            current = parent.parent();
        }
        for dir in ancestors.into_iter().rev() {
            if dirs_written.insert(dir.clone()) {
                write_newc_header(&mut gz, &format!("./{}", dir), 0o040755, 0)?;
                pad_writer(&mut gz, 0)?;
            }
        }
    }

    for entry in entries {
        let store_path = format!("./{}", entry.rel.replace('\\', "/"));
        write_newc_header(&mut gz, &store_path, 0o100644, entry.size as usize)?;

        let mut src = std::fs::File::open(&entry.abs)
            .with_context(|| format!("open {}", entry.abs.display()))?;
        let mut buf = [0u8; UPLOAD_CHUNK];
        let mut copied = 0usize;
        loop {
            let n = src.read(&mut buf)?;
            if n == 0 {
                break;
            }
            gz.write_all(&buf[..n])?;
            copied += n;
        }
        if copied != entry.size as usize {
            // File changed while reading; pad so the archive stays valid.
            let missing = (entry.size as usize).saturating_sub(copied);
            gz.write_all(&vec![0u8; missing])?;
        }
        pad_writer(&mut gz, entry.size as usize)?;
    }

    // Trailer record ends the archive.
    write_newc_header(&mut gz, "TRAILER!!!", 0, 0)?;
    pad_writer(&mut gz, 0)?;

    gz.finish()?.into_inner().map_err(|e| anyhow!("{}", e))?;
    Ok(())
}

fn write_newc_header<W: Write>(out: &mut W, name: &str, mode: u32, filesize: usize) -> Result<()> {
    let name_bytes = name.as_bytes();
    let namesize = name_bytes.len() + 1;

    let mut header = Vec::with_capacity(110 + namesize + 3);
    header.extend_from_slice(b"070701");
    let fields: [usize; 13] = [
        0,               // ino
        mode as usize,   // mode
        0,               // uid
        0,               // gid
        1,               // nlink
        0,               // mtime
        filesize,        // filesize
        0,               // devmajor
        0,               // devminor
        0,               // rdevmajor
        0,               // rdevminor
        namesize,        // namesize
        0,               // check
    ];
    for field in fields {
        header.extend_from_slice(format!("{:08x}", field).as_bytes());
    }
    out.write_all(&header)?;
    out.write_all(name_bytes)?;
    out.write_all(&[0])?;
    // Header (110) + name + NUL padded to 4 bytes.
    let written = 110 + namesize;
    let pad = (4 - (written % 4)) % 4;
    out.write_all(&[0u8; 3][..pad])?;
    Ok(())
}

fn pad_writer<W: Write>(out: &mut W, datasize: usize) -> Result<()> {
    let pad = (4 - (datasize % 4)) % 4;
    out.write_all(&[0u8; 3][..pad])?;
    Ok(())
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

/// Stream the archive file as a chunked /Upload, reporting progress.
async fn post_upload_streamed(
    target: SocketAddr,
    archive_path: &Path,
    progress: &SendProgress,
) -> Result<()> {
    let total = tokio::fs::metadata(archive_path).await?.len().max(1);

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

    let mut src = tokio::fs::File::open(archive_path).await?;
    let mut chunk = vec![0u8; UPLOAD_CHUNK];
    let mut sent: u64 = 0;
    loop {
        let n = src.read(&mut chunk).await?;
        if n == 0 {
            break;
        }
        let header = format!("{:x}\r\n", n);
        tls.write_all(header.as_bytes()).await?;
        tls.write_all(&chunk[..n]).await?;
        tls.write_all(b"\r\n").await?;
        sent += n as u64;
        // Hold a little back so the bar does not sit at 100% while the
        // receiver is still writing to disk.
        set_progress(progress, (sent as f32 / total as f32 * 98.0).min(98.0));
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
        "audio/mpeg" => "public.mp3".to_string(),
        "audio/wav" | "audio/x-wav" => "com.microsoft.waveform-audio".to_string(),
        "audio/aiff" | "audio/x-aiff" => "public.aiff-audio".to_string(),
        "audio/mp4" => "public.mpeg-4-audio".to_string(),
        "audio/flac" => "org.xiph.flac".to_string(),
        _ => "public.content".to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Files and folders survive the archive → extract round trip intact.
    #[test]
    fn cpio_roundtrip_preserves_files_and_folders() {
        let src = std::env::temp_dir().join(format!("airdropd-test-src-{}", uuid::Uuid::new_v4()));
        let out = std::env::temp_dir().join(format!("airdropd-test-out-{}", uuid::Uuid::new_v4()));
        let set_dir = src.join("My Set");
        std::fs::create_dir_all(set_dir.join("stems")).unwrap();
        std::fs::write(src.join("track.wav"), vec![7u8; 100_000]).unwrap();
        std::fs::write(set_dir.join("mix.mp3"), b"mixdata".to_vec()).unwrap();
        std::fs::write(set_dir.join("stems/kick.wav"), b"kick".to_vec()).unwrap();

        let entries =
            collect_entries(&[src.join("track.wav"), set_dir.clone()]).unwrap();
        assert_eq!(entries.len(), 3);

        let archive = std::env::temp_dir().join(format!("airdropd-test-{}.cpio.gz", uuid::Uuid::new_v4()));
        build_archive(&entries, &archive).unwrap();

        std::fs::create_dir_all(&out).unwrap();
        let reader = std::fs::File::open(&archive).unwrap();
        let saved = crate::protocols::cpio_extract::extract_gzip_cpio(reader, &out).unwrap();
        assert_eq!(saved.len(), 3);

        assert_eq!(std::fs::read(out.join("track.wav")).unwrap().len(), 100_000);
        assert_eq!(std::fs::read(out.join("My Set/mix.mp3")).unwrap(), b"mixdata");
        assert_eq!(std::fs::read(out.join("My Set/stems/kick.wav")).unwrap(), b"kick");

        let _ = std::fs::remove_dir_all(&src);
        let _ = std::fs::remove_dir_all(&out);
        let _ = std::fs::remove_file(&archive);
    }

    /// /Ask plist must list every file and carry the link when present.
    #[test]
    fn ask_plist_contains_files_and_link() {
        let entries = vec![
            SendEntry {
                abs: PathBuf::from("a.mp3"),
                rel: "a.mp3".to_string(),
                size: 10,
            },
            SendEntry {
                abs: PathBuf::from("set/b.wav"),
                rel: "set/b.wav".to_string(),
                size: 20,
            },
        ];
        let bytes = build_ask_plist("DJ-PC", "abcdef", &entries, None).unwrap();
        let value: Value = plist::from_bytes(&bytes).unwrap();
        let dict = value.as_dictionary().unwrap();
        let files = dict.get("Files").unwrap().as_array().unwrap();
        assert_eq!(files.len(), 2);
        let second = files[1].as_dictionary().unwrap();
        assert_eq!(
            second.get("FileBomPath").unwrap().as_string().unwrap(),
            "./set/b.wav"
        );

        let link_bytes =
            build_ask_plist("DJ-PC", "abcdef", &[], Some("https://example.com")).unwrap();
        let link_value: Value = plist::from_bytes(&link_bytes).unwrap();
        let link_dict = link_value.as_dictionary().unwrap();
        let items = link_dict.get("Items").unwrap().as_array().unwrap();
        assert_eq!(items[0].as_string().unwrap(), "https://example.com");
    }
}
