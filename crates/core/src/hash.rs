use crate::header::{detect_header, HeaderInfo, RomHeaderKind};
use md5::Md5;
use retrotools_common::error::{AppError, AppResult};
use sha1::Sha1;
use sha2::Digest;
use sha2::Sha256;
use std::io::Read;

const CHUNK_SIZE: usize = 64 * 1024;
const HEADER_PROBE_SIZE: usize = 128;

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FileHashes {
    pub size: u64,
    pub crc32: String,
    pub md5: String,
    pub sha1: String,
    pub sha256: String,
}

#[derive(Debug, Clone)]
pub struct HashResult {
    pub full: FileHashes,
    pub headerless: Option<FileHashes>,
    pub header: HeaderInfo,
}

struct HasherSet {
    crc: crc32fast::Hasher,
    md5: Md5,
    sha1: Sha1,
    sha256: Sha256,
    len: u64,
}

impl HasherSet {
    fn new() -> Self {
        Self {
            crc: crc32fast::Hasher::new(),
            md5: Md5::new(),
            sha1: Sha1::new(),
            sha256: Sha256::new(),
            len: 0,
        }
    }

    fn update(&mut self, data: &[u8]) {
        self.crc.update(data);
        self.md5.update(data);
        self.sha1.update(data);
        self.sha256.update(data);
        self.len += data.len() as u64;
    }

    fn finish(self) -> FileHashes {
        FileHashes {
            size: self.len,
            crc32: format!("{:08x}", self.crc.finalize()),
            md5: to_hex(&self.md5.finalize()),
            sha1: to_hex(&self.sha1.finalize()),
            sha256: to_hex(&self.sha256.finalize()),
        }
    }
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn read_probe<R: Read>(reader: &mut R, buf: &mut [u8]) -> AppResult<usize> {
    let mut total = 0;
    while total < buf.len() {
        let n = reader.read(&mut buf[total..]).map_err(AppError::Io)?;
        if n == 0 {
            break;
        }
        total += n;
    }
    Ok(total)
}

/// Streams `reader` once, computing CRC32/MD5/SHA1/SHA256 over the full content.
/// If a known ROM container header is detected in the leading bytes, a second
/// set of "headerless" hashes (header stripped) is computed in the same pass.
pub fn compute_hashes<R: Read>(mut reader: R) -> AppResult<HashResult> {
    let mut probe = vec![0u8; HEADER_PROBE_SIZE];
    let probe_read = read_probe(&mut reader, &mut probe)?;
    probe.truncate(probe_read);

    let header = detect_header(&probe);

    let mut full = HasherSet::new();
    let mut headerless = if header.kind != RomHeaderKind::None {
        Some(HasherSet::new())
    } else {
        None
    };

    full.update(&probe);
    if let Some(hl) = headerless.as_mut() {
        if probe.len() > header.header_size {
            hl.update(&probe[header.header_size..]);
        }
    }

    let mut buf = vec![0u8; CHUNK_SIZE];
    loop {
        let n = reader.read(&mut buf).map_err(AppError::Io)?;
        if n == 0 {
            break;
        }
        full.update(&buf[..n]);
        if let Some(hl) = headerless.as_mut() {
            hl.update(&buf[..n]);
        }
    }

    Ok(HashResult {
        full: full.finish(),
        headerless: headerless.map(HasherSet::finish),
        header,
    })
}

pub fn compute_hashes_for_file(path: &std::path::Path) -> AppResult<HashResult> {
    let file = std::fs::File::open(path).map_err(AppError::Io)?;
    compute_hashes(std::io::BufReader::new(file))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_plain_data() {
        let data = b"hello world";
        let result = compute_hashes(&data[..]).unwrap();
        assert_eq!(result.full.size, 11);
        assert_eq!(result.full.crc32, "0d4a1185");
        assert!(result.headerless.is_none());
    }

    #[test]
    fn strips_ines_header_for_headerless_hash() {
        let mut data = vec![b'N', b'E', b'S', 0x1A];
        data.extend(vec![0u8; 12]);
        data.extend(b"PAYLOAD-BYTES");
        let result = compute_hashes(&data[..]).unwrap();
        assert_eq!(result.full.size, data.len() as u64);
        let headerless = result.headerless.unwrap();
        assert_eq!(headerless.size, data.len() as u64 - 16);
        assert_ne!(headerless.crc32, result.full.crc32);
    }

    #[test]
    fn identical_content_yields_identical_hashes() {
        let a = compute_hashes(&b"same content"[..]).unwrap();
        let b = compute_hashes(&b"same content"[..]).unwrap();
        assert_eq!(a.full, b.full);
    }
}
