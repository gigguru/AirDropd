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

        let namesize = parse_hex_field(&hdr[94..108])?;
        let filesize = parse_hex_field(&hdr[54..64])?;

        let mut name_buf = vec![0u8; namesize];
        reader.read_exact(&mut name_buf).context("read cpio name")?;
        let name = String::from_utf8_lossy(&name_buf)
            .trim_end_matches('\0')
            .to_string();

        pad_read(&mut reader, namesize)?;

        if name == "TRAILER!!!" {
            break;
        }

        let safe_name = sanitize_filename(Path::new(&name).file_name().and_then(|n| n.to_str()).unwrap_or("file"));
        let out_path = unique_path(dest_dir, &safe_name);

        if filesize > 0 {
            let mut data = vec![0u8; filesize];
            reader.read_exact(&mut data).context("read cpio file data")?;
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&out_path, &data)?;
            saved.push(out_path);
        }

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
