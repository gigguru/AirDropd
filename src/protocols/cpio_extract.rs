//! Extract gzip-compressed newc cpio archives from AirDrop /Upload requests.

use anyhow::{anyhow, Context, Result};
use flate2::read::GzDecoder;
use std::io::Read;
use std::path::{Path, PathBuf};

const NEWC_MAGIC: &[u8; 6] = b"070701";

pub fn extract_gzip_cpio<R: Read>(reader: R, dest_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut peek = [0u8; 6];
    let mut source = reader;
    source.read_exact(&mut peek).context("read upload header")?;

    let payload: Box<dyn Read> = if &peek == NEWC_MAGIC {
        Box::new(ReadChain::new(peek.to_vec(), source))
    } else if peek[0..2] == [0x1f, 0x8b] {
        let mut gz_data = peek.to_vec();
        source.read_to_end(&mut gz_data)?;
        Box::new(GzDecoder::new(std::io::Cursor::new(gz_data)))
    } else {
        return Err(anyhow!(
            "unsupported upload format (expected gzip cpio or newc cpio)"
        ));
    };

    extract_newc_cpio(payload, dest_dir)
}

struct ReadChain<R: Read> {
    prefix: std::io::Cursor<Vec<u8>>,
    rest: R,
}

impl<R: Read> ReadChain<R> {
    fn new(prefix: Vec<u8>, rest: R) -> Self {
        Self {
            prefix: std::io::Cursor::new(prefix),
            rest,
        }
    }
}

impl<R: Read> Read for ReadChain<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n1 = self.prefix.read(buf)?;
        if n1 > 0 {
            return Ok(n1);
        }
        self.rest.read(buf)
    }
}

fn extract_newc_cpio<R: Read>(mut reader: R, dest_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut saved = Vec::new();

    loop {
        let mut hdr = [0u8; 110];
        if let Err(e) = reader.read_exact(&mut hdr) {
            if saved.is_empty() {
                return Err(e.into());
            }
            break;
        }

        if &hdr[..6] != NEWC_MAGIC {
            return Err(anyhow!("invalid cpio magic"));
        }

        // newc header: 8-hex-char fields. mode @14, filesize @54, namesize @94.
        let mode = parse_hex_field(&hdr[14..22]).unwrap_or(0);
        let filesize = parse_hex_field(&hdr[54..62])?;
        let namesize = parse_hex_field(&hdr[94..102])?;

        let mut name_buf = vec![0u8; namesize];
        reader.read_exact(&mut name_buf).context("read cpio name")?;
        let name = String::from_utf8_lossy(&name_buf)
            .trim_end_matches('\0')
            .to_string();

        // Header (110) + name is padded to a 4-byte boundary.
        pad_read(&mut reader, 110 + namesize)?;

        if name == "TRAILER!!!" {
            break;
        }

        let is_dir = mode & 0o170000 == 0o040000;
        let rel_path = sanitize_relative_path(&name);

        if is_dir || (filesize == 0 && name.ends_with('/')) {
            if let Some(rel) = &rel_path {
                let _ = std::fs::create_dir_all(dest_dir.join(rel));
            }
            pad_read_data(&mut reader, filesize, &mut Vec::new())?;
            continue;
        }

        let out_path = match &rel_path {
            // Preserve folder structure; pick a unique name only for the leaf.
            Some(rel) => {
                let parent_rel = rel.parent().map(|p| p.to_path_buf()).unwrap_or_default();
                let parent_abs = dest_dir.join(&parent_rel);
                std::fs::create_dir_all(&parent_abs)?;
                let leaf = rel
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("received_file");
                unique_path(&parent_abs, leaf)
            }
            None => unique_path(dest_dir, "received_file"),
        };

        let mut remaining = filesize;
        let mut out = std::fs::File::create(&out_path)
            .with_context(|| format!("create {}", out_path.display()))?;
        let mut buf = [0u8; 65536];
        while remaining > 0 {
            let take = remaining.min(buf.len());
            reader
                .read_exact(&mut buf[..take])
                .context("read cpio file data")?;
            std::io::Write::write_all(&mut out, &buf[..take])?;
            remaining -= take;
        }
        saved.push(out_path);

        pad_read(&mut reader, filesize)?;
    }

    Ok(saved)
}

fn parse_hex_field(bytes: &[u8]) -> Result<usize> {
    let s = std::str::from_utf8(bytes).context("cpio hex field")?;
    usize::from_str_radix(s, 16).context("parse cpio hex field")
}

fn pad_read<R: Read>(reader: &mut R, size: usize) -> Result<()> {
    let pad = (4 - (size % 4)) % 4;
    if pad == 0 {
        return Ok(());
    }
    let mut skip = [0u8; 3];
    reader.read_exact(&mut skip[..pad])?;
    Ok(())
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| if r#"<>:"/\|?*"#.contains(c) { '_' } else { c })
        .collect()
}

/// Convert an archive path into a safe relative path: strips `./` prefixes,
/// rejects traversal components, and sanitizes every segment for Windows.
fn sanitize_relative_path(name: &str) -> Option<PathBuf> {
    let cleaned = name.trim_start_matches("./").replace('\\', "/");
    let mut out = PathBuf::new();
    for part in cleaned.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            return None;
        }
        out.push(sanitize_filename(part));
    }
    if out.as_os_str().is_empty() {
        None
    } else {
        Some(out)
    }
}

/// Consume and discard `size` data bytes plus padding (for directory records).
fn pad_read_data<R: Read>(reader: &mut R, size: usize, _scratch: &mut Vec<u8>) -> Result<()> {
    let mut remaining = size;
    let mut buf = [0u8; 4096];
    while remaining > 0 {
        let take = remaining.min(buf.len());
        reader.read_exact(&mut buf[..take])?;
        remaining -= take;
    }
    pad_read(reader, size)
}

fn unique_path(dir: &Path, filename: &str) -> PathBuf {
    let base = dir.join(if filename.is_empty() { "received_file" } else { filename });
    if !base.exists() {
        return base;
    }
    let stem = base.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    let ext = base.extension().and_then(|s| s.to_str());
    for i in 1..1000 {
        let candidate = match ext {
            Some(e) => dir.join(format!("{} ({}).{}", stem, i, e)),
            None => dir.join(format!("{} ({})", stem, i)),
        };
        if !candidate.exists() {
            return candidate;
        }
    }
    dir.join(format!("{}_{}", stem, uuid::Uuid::new_v4()))
}

/// Read HTTP chunked transfer-encoded body bytes.
pub struct ChunkedBodyReader<R: Read> {
    inner: R,
    chunk: Vec<u8>,
    done: bool,
}

impl<R: Read> ChunkedBodyReader<R> {
    pub fn new(inner: R) -> Self {
        Self {
            inner,
            chunk: Vec::new(),
            done: false,
        }
    }

    fn next_chunk(&mut self) -> Result<()> {
        if self.done {
            self.chunk.clear();
            return Ok(());
        }
        if !self.chunk.is_empty() {
            return Ok(());
        }

        let mut size_line = Vec::new();
        loop {
            let mut b = [0u8; 1];
            self.inner.read_exact(&mut b)?;
            if b[0] == b'\n' {
                break;
            }
            if b[0] != b'\r' {
                size_line.push(b[0]);
            }
        }

        let size_str = String::from_utf8_lossy(&size_line);
        let size = usize::from_str_radix(size_str.trim(), 16).context("chunk size")?;
        if size == 0 {
            self.done = true;
            self.chunk.clear();
            // trailing CRLF after final chunk
            let mut trailer = [0u8; 2];
            let _ = self.inner.read(&mut trailer);
            return Ok(());
        }

        let mut buf = vec![0u8; size];
        self.inner.read_exact(&mut buf)?;
        self.chunk = buf;

        let mut crlf = [0u8; 2];
        self.inner.read_exact(&mut crlf)?;
        Ok(())
    }
}

impl<R: Read> Read for ChunkedBodyReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.next_chunk().map_err(|e| std::io::Error::other(e.to_string()))?;
        if self.chunk.is_empty() {
            return Ok(0);
        }
        let n = self.chunk.len().min(buf.len());
        buf[..n].copy_from_slice(&self.chunk[..n]);
        self.chunk.drain(..n);
        Ok(n)
    }
}
