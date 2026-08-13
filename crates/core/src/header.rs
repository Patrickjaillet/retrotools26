#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RomHeaderKind {
    None,
    INes,
    Lynx,
    Atari7800,
}

impl std::fmt::Display for RomHeaderKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            RomHeaderKind::None => "none",
            RomHeaderKind::INes => "iNES",
            RomHeaderKind::Lynx => "LYNX",
            RomHeaderKind::Atari7800 => "A78",
        };
        write!(f, "{label}")
    }
}

#[derive(Debug, Clone, Copy)]
pub struct HeaderInfo {
    pub kind: RomHeaderKind,
    pub header_size: usize,
}

impl HeaderInfo {
    pub fn none() -> Self {
        Self {
            kind: RomHeaderKind::None,
            header_size: 0,
        }
    }
}

/// Detects a known ROM container header from the leading bytes of a file.
/// Callers only need to provide the first 128 bytes (or fewer, for small files).
pub fn detect_header(bytes: &[u8]) -> HeaderInfo {
    if bytes.len() >= 4 && &bytes[0..4] == b"NES\x1A" {
        return HeaderInfo {
            kind: RomHeaderKind::INes,
            header_size: 16,
        };
    }
    if bytes.len() >= 4 && &bytes[0..4] == b"LYNX" {
        return HeaderInfo {
            kind: RomHeaderKind::Lynx,
            header_size: 64,
        };
    }
    if bytes.len() >= 10 && &bytes[1..10] == b"ATARI7800" {
        return HeaderInfo {
            kind: RomHeaderKind::Atari7800,
            header_size: 128,
        };
    }
    HeaderInfo::none()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_ines_header() {
        let mut data = vec![b'N', b'E', b'S', 0x1A];
        data.extend(vec![0u8; 12]);
        let info = detect_header(&data);
        assert_eq!(info.kind, RomHeaderKind::INes);
        assert_eq!(info.header_size, 16);
    }

    #[test]
    fn detects_lynx_header() {
        let mut data = b"LYNX".to_vec();
        data.extend(vec![0u8; 60]);
        let info = detect_header(&data);
        assert_eq!(info.kind, RomHeaderKind::Lynx);
        assert_eq!(info.header_size, 64);
    }

    #[test]
    fn detects_a78_header() {
        let mut data = vec![0x01];
        data.extend(b"ATARI7800");
        data.extend(vec![0u8; 118]);
        let info = detect_header(&data);
        assert_eq!(info.kind, RomHeaderKind::Atari7800);
        assert_eq!(info.header_size, 128);
    }

    #[test]
    fn no_header_for_plain_data() {
        let data = vec![0u8; 32];
        let info = detect_header(&data);
        assert_eq!(info.kind, RomHeaderKind::None);
        assert_eq!(info.header_size, 0);
    }
}
