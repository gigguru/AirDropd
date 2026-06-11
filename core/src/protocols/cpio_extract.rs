//! Extract gzip-compressed cpio archives from AirDrop /Upload requests.
//!
//! Apple builds AirDrop archives with libarchive, whose default cpio writer
//! emits the old portable ASCII format ("odc", magic `070707`) — that is what
//! OpenDrop sends and what Apple devices produce. Some implementations use
//! the newer "newc" format (magic `070701`). Both are supported here.

use anyhow::{anyhow, Context, Result};
use flate2::read::GzDecoder;
use std::io::Read;
use std::path::{Path, PathBuf};

const NEWC_MAGIC: &[u8; 6] = b"070701";
const ODC_MAGIC: &[u8; 6] = b"070707";

pub fn extract_gzip_cpio<R: Read>(reader: R, dest_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut peek = [0u8; 6];
    let mut source = reader;
    source.read_exact(&mut peek).context("read upload header")?;

    let payload: Box<dyn Read> = if &peek == NEWC_MAGIC || &peek == ODC_MAGIC {
        Box::new(ReadChain::new(peek.to_vec(), source))
    } else if peek[0..2] == [0x1f, 0x8b] {
        let mut gz_data = peek.to_vec();
        source.read_to_end(&mut gz_data)?;
        Box::new(GzDecoder::new(std::io::Cursor::new(gz_data)))
    } else {
        return Err(anyhow!(
            "unsupported upload format (expected gzip cpio, odc cpio, or newc cpio)"
        ));
    };

    extract_cpio(payload, dest_dir)
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

/// One decoded cpio record header (format differences abstracted away).
struct CpioRecord {
    name: String,
    mode: usize,
    filesize: usize,
    /// newc pads name and data to 4-byte boundaries; odc has no padding.
    pad4: bool,
}

fn extract_cpio<R: Read>(mut reader: R, dest_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut saved = Vec::new();

    loop {
        let mut magic = [0u8; 6];
        if let Err(e) = reader.read_exact(&mut magic) {
            if saved.is_empty() {
                return Err(e.into());
            }
            break;
        }

        let record = if &magic == NEWC_MAGIC {
            read_newc_record(&mut reader)?
        } else if &magic == ODC_MAGIC {
            read_odc_record(&mut reader)?
        } else {
            return Err(anyhow!("invalid cpio magic"));
        };

        if record.name == "TRAILER!!!" {
            break;
        }

        let is_dir = record.mode & 0o170000 == 0o040000;
        let rel_path = sanitize_relative_path(&record.name);

        if is_dir || (record.filesize == 0 && record.name.ends_with('/')) {
            if let Some(rel) = &rel_path {
                let _ = std::fs::create_dir_all(dest_dir.join(rel));
            }
            skip_data(&mut reader, record.filesize, record.pad4)?;
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

        let mut remaining = record.filesize;
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

        if record.pad4 {
            pad_read(&mut reader, record.filesize)?;
        }
    }

    Ok(saved)
}

/// newc: 110-byte header of 8-hex-char fields (magic already consumed).
fn read_newc_record<R: Read>(reader: &mut R) -> Result<CpioRecord> {
    let mut hdr = [0u8; 104];
    reader.read_exact(&mut hdr).context("read newc header")?;

    // Offsets relative to the full header: mode @14, filesize @54, namesize @94.
    let mode = parse_hex_field(&hdr[8..16]).unwrap_or(0);
    let filesize = parse_hex_field(&hdr[48..56])?;
    let namesize = parse_hex_field(&hdr[88..96])?;

    let name = read_name(reader, namesize)?;
    // Header (110) + name is padded to a 4-byte boundary.
    pad_read(reader, 110 + namesize)?;

    Ok(CpioRecord {
        name,
        mode,
        filesize,
        pad4: true,
    })
}

/// odc (old portable ASCII, libarchive/Apple default): 76-byte header of
/// octal fields, no padding anywhere (magic already consumed).
fn read_odc_record<R: Read>(reader: &mut R) -> Result<CpioRecord> {
    let mut hdr = [0u8; 70];
    reader.read_exact(&mut hdr).context("read odc header")?;

    // After magic: dev[6] ino[6] mode[6] uid[6] gid[6] nlink[6] rdev[6]
    //              mtime[11] namesize[6] filesize[11]
    let mode = parse_octal_field(&hdr[12..18]).unwrap_or(0);
    let namesize = parse_octal_field(&hdr[53..59])?;
    let filesize = parse_octal_field(&hdr[59..70])?;

    let name = read_name(reader, namesize)?;

    Ok(CpioRecord {
        name,
        mode,
        filesize,
        pad4: false,
    })
}

fn read_name<R: Read>(reader: &mut R, namesize: usize) -> Result<String> {
    if namesize == 0 || namesize > 4096 {
        return Err(anyhow!("invalid cpio name size {}", namesize));
    }
    let mut name_buf = vec![0u8; namesize];
    reader.read_exact(&mut name_buf).context("read cpio name")?;
    Ok(String::from_utf8_lossy(&name_buf)
        .trim_end_matches('\0')
        .to_string())
}

fn parse_hex_field(bytes: &[u8]) -> Result<usize> {
    let s = std::str::from_utf8(bytes).context("cpio hex field")?;
    usize::from_str_radix(s, 16).context("parse cpio hex field")
}

fn parse_octal_field(bytes: &[u8]) -> Result<usize> {
    let s = std::str::from_utf8(bytes).context("cpio octal field")?;
    usize::from_str_radix(s.trim(), 8).context("parse cpio octal field")
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

/// Consume and discard `size` data bytes (plus newc padding when `pad4`).
fn skip_data<R: Read>(reader: &mut R, size: usize, pad4: bool) -> Result<()> {
    let mut remaining = size;
    let mut buf = [0u8; 4096];
    while remaining > 0 {
        let take = remaining.min(buf.len());
        reader.read_exact(&mut buf[..take])?;
        remaining -= take;
    }
    if pad4 {
        pad_read(reader, size)?;
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    fn newc_record(name: &str, mode: usize, data: &[u8]) -> Vec<u8> {
        let namesize = name.len() + 1;
        let mut out = Vec::new();
        out.extend_from_slice(b"070701");
        let fields: [usize; 13] = [
            0, mode, 0, 0, 1, 0, data.len(), 0, 0, 0, 0, namesize, 0,
        ];
        for f in fields {
            out.extend_from_slice(format!("{:08x}", f).as_bytes());
        }
        out.extend_from_slice(name.as_bytes());
        out.push(0);
        let pad = (4 - ((110 + namesize) % 4)) % 4;
        out.extend_from_slice(&[0u8; 3][..pad]);
        out.extend_from_slice(data);
        let pad = (4 - (data.len() % 4)) % 4;
        out.extend_from_slice(&[0u8; 3][..pad]);
        out
    }

    /// Some senders use the newc format — extraction must still work after
    /// the switch to odc on the send side.
    #[test]
    fn extracts_newc_archives_with_folders() {
        let mut archive = Vec::new();
        archive.extend(newc_record("./set", 0o040755, b""));
        archive.extend(newc_record("./set/track.mp3", 0o100644, b"beat drop"));
        archive.extend(newc_record("TRAILER!!!", 0, b""));

        let dest = std::env::temp_dir().join(format!("airdropd-newc-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dest).unwrap();

        let saved = extract_cpio(std::io::Cursor::new(archive), &dest).unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!(std::fs::read(&saved[0]).unwrap(), b"beat drop");
        assert!(saved[0].parent().unwrap().ends_with("set"));

        let _ = std::fs::remove_dir_all(&dest);
    }

    fn odc_record(name: &str, mode: usize, data: &[u8]) -> Vec<u8> {
        let namesize = name.len() + 1;
        let mut out = Vec::new();
        out.extend_from_slice(
            format!(
                "070707{:06o}{:06o}{:06o}{:06o}{:06o}{:06o}{:06o}{:011o}{:06o}{:011o}",
                0, 1, mode, 0, 0, 1, 0, 0, namesize, data.len()
            )
            .as_bytes(),
        );
        out.extend_from_slice(name.as_bytes());
        out.push(0);
        out.extend_from_slice(data);
        out
    }

    /// Apple devices (libarchive) send odc archives — the format AirDrop
    /// actually uses on the wire.
    #[test]
    fn extracts_odc_archives_with_folders() {
        let mut archive = Vec::new();
        archive.extend(odc_record("./photos", 0o040755, b""));
        archive.extend(odc_record("./photos/img.heic", 0o100644, b"pixels"));
        archive.extend(odc_record("TRAILER!!!", 0, b""));

        let dest = std::env::temp_dir().join(format!("airdropd-odc-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dest).unwrap();

        let saved = extract_cpio(std::io::Cursor::new(archive), &dest).unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!(std::fs::read(&saved[0]).unwrap(), b"pixels");
        assert!(saved[0].parent().unwrap().ends_with("photos"));

        let _ = std::fs::remove_dir_all(&dest);
    }
}
