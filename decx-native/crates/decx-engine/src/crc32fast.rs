//! IEEE CRC-32 for decx-engine (same polynomial as crc32fast/zlib).
//! Zero external deps; table built once via OnceLock.

use std::sync::OnceLock;

fn table() -> &'static [u32; 256] {
    static TABLE: OnceLock<[u32; 256]> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut t = [0u32; 256];
        for i in 0..256u32 {
            let mut c = i;
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xEDB88320 ^ (c >> 1) } else { c >> 1 };
            }
            t[i as usize] = c;
        }
        t
    })
}

/// IEEE CRC-32 checksum, compatible with crc32fast::hash / zlib crc32.
pub fn hash(data: &[u8]) -> u32 {
    let t = table();
    let mut c: u32 = 0xFFFFFFFF;
    for &b in data {
        c = t[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8);
    }
    c ^ 0xFFFFFFFF
}

#[cfg(test)]
mod tests {
    use super::hash;

    #[test]
    fn known_vectors() {
        assert_eq!(hash(b""), 0);
        assert_eq!(hash(b"a"), 0xE8B7BE43);
        assert_eq!(hash(b"abc"), 0x352441C2);
        assert_eq!(hash(b"123456789"), 0xCBF43926);
        assert_eq!(hash(b"decx-native"), hash(b"decx-native"));
    }
}
