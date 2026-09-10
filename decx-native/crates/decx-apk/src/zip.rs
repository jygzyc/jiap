//! Minimal ZIP reader for APK/JAR containers: central directory driven,
//! stored (0) + deflate (8) methods. No zip64 (APKs < 4 GiB).

use crate::inflate::inflate;

pub struct ZipEntry {
    pub name: String,
    pub method: u16,
    pub compressed_size: u64,
    pub uncompressed_size: u64,
    pub data_offset: u64,
    pub crc32: u32,
}

pub struct ZipArchive {
    data: Vec<u8>,
    entries: Vec<ZipEntry>,
}

fn u16le(d: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([d[off], d[off + 1]])
}
fn u32le(d: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([d[off], d[off + 1], d[off + 2], d[off + 3]])
}

impl ZipArchive {
    pub fn open(data: Vec<u8>) -> Result<ZipArchive, String> {
        let eocd = find_eocd(&data).ok_or("zip: end of central directory not found")?;
        let cd_count = u16le(&data, eocd + 10) as usize;
        let cd_offset = u32le(&data, eocd + 16) as usize;

        let mut entries = Vec::with_capacity(cd_count);
        let mut pos = cd_offset;
        for _ in 0..cd_count {
            if u32le(&data, pos) != 0x02014b50 {
                return Err("zip: bad central directory entry".into());
            }
            let method = u16le(&data, pos + 10);
            let crc32 = u32le(&data, pos + 16);
            let csize = u32le(&data, pos + 20) as u64;
            let usize_ = u32le(&data, pos + 24) as u64;
            let name_len = u16le(&data, pos + 28) as usize;
            let extra_len = u16le(&data, pos + 30) as usize;
            let comment_len = u16le(&data, pos + 32) as usize;
            let lho = u32le(&data, pos + 42) as usize;
            let name = String::from_utf8_lossy(
                data.get(pos + 46..pos + 46 + name_len).ok_or("zip: name oob")?,
            )
            .into_owned();
            // local header: skip its name/extra to find data start
            if u32le(&data, lho) != 0x04034b50 {
                return Err("zip: bad local header".into());
            }
            let lname_len = u16le(&data, lho + 26) as usize;
            let lextra_len = u16le(&data, lho + 28) as usize;
            let data_offset = (lho + 30 + lname_len + lextra_len) as u64;
            entries.push(ZipEntry {
                name,
                method,
                compressed_size: csize,
                uncompressed_size: usize_,
                data_offset,
                crc32,
            });
            pos += 46 + name_len + extra_len + comment_len;
        }
        Ok(ZipArchive { data, entries })
    }

    pub fn entries(&self) -> &[ZipEntry] {
        &self.entries
    }

    pub fn entry_names(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.name.as_str()).collect()
    }

    pub fn find(&self, name: &str) -> Option<&ZipEntry> {
        self.entries.iter().find(|e| e.name == name)
    }

    pub fn read(&self, entry: &ZipEntry) -> Result<Vec<u8>, String> {
        let start = entry.data_offset as usize;
        let end = start + entry.compressed_size as usize;
        let raw = self.data.get(start..end).ok_or("zip: data out of bounds")?;
        match entry.method {
            0 => Ok(raw.to_vec()),
            8 => {
                let mut out = Vec::with_capacity(entry.uncompressed_size as usize);
                inflate(raw, &mut out)?;
                if out.len() as u64 != entry.uncompressed_size {
                    return Err(format!(
                        "zip: size mismatch (expected {}, got {})",
                        entry.uncompressed_size,
                        out.len()
                    ));
                }
                Ok(out)
            }
            m => Err(format!("zip: unsupported method {m}")),
        }
    }

    pub fn read_by_name(&self, name: &str) -> Option<Result<Vec<u8>, String>> {
        self.find(name).map(|e| self.read(e))
    }
}

fn find_eocd(data: &[u8]) -> Option<usize> {
    if data.len() < 22 {
        return None;
    }
    let mut i = data.len() - 22;
    loop {
        if u32le(data, i) == 0x06054b50 {
            return Some(i);
        }
        if i == 0 {
            return None;
        }
        i -= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_stored_zip() {
        // hand-built zip: one stored entry "a.txt" -> "hi"
        let name = b"a.txt";
        let mut z: Vec<u8> = Vec::new();
        // local header
        z.extend_from_slice(&0x04034b50u32.to_le_bytes());
        z.extend_from_slice(&20u16.to_le_bytes()); // version
        z.extend_from_slice(&0u16.to_le_bytes()); // flags
        z.extend_from_slice(&0u16.to_le_bytes()); // method stored
        z.extend_from_slice(&0u16.to_le_bytes()); // time
        z.extend_from_slice(&0u16.to_le_bytes()); // date
        z.extend_from_slice(&0u32.to_le_bytes()); // crc (unchecked in this test)
        z.extend_from_slice(&2u32.to_le_bytes()); // csize
        z.extend_from_slice(&2u32.to_le_bytes()); // usize
        z.extend_from_slice(&(name.len() as u16).to_le_bytes());
        z.extend_from_slice(&0u16.to_le_bytes()); // extra len
        z.extend_from_slice(name);
        z.extend_from_slice(b"hi");
        let lho = 0usize;
        // central directory
        let cd_start = z.len();
        z.extend_from_slice(&0x02014b50u32.to_le_bytes());
        z.extend_from_slice(&20u16.to_le_bytes());
        z.extend_from_slice(&20u16.to_le_bytes());
        z.extend_from_slice(&0u16.to_le_bytes()); // flags
        z.extend_from_slice(&0u16.to_le_bytes()); // method
        z.extend_from_slice(&0u16.to_le_bytes());
        z.extend_from_slice(&0u16.to_le_bytes());
        z.extend_from_slice(&0u32.to_le_bytes()); // crc
        z.extend_from_slice(&2u32.to_le_bytes());
        z.extend_from_slice(&2u32.to_le_bytes());
        z.extend_from_slice(&(name.len() as u16).to_le_bytes());
        z.extend_from_slice(&0u16.to_le_bytes()); // extra
        z.extend_from_slice(&0u16.to_le_bytes()); // comment
        z.extend_from_slice(&0u16.to_le_bytes()); // disk
        z.extend_from_slice(&0u16.to_le_bytes()); // int attrs
        z.extend_from_slice(&0u32.to_le_bytes()); // ext attrs
        z.extend_from_slice(&(lho as u32).to_le_bytes());
        z.extend_from_slice(name);
        let cd_size = z.len() - cd_start;
        // eocd
        z.extend_from_slice(&0x06054b50u32.to_le_bytes());
        z.extend_from_slice(&0u16.to_le_bytes());
        z.extend_from_slice(&0u16.to_le_bytes());
        z.extend_from_slice(&1u16.to_le_bytes()); // entries
        z.extend_from_slice(&1u16.to_le_bytes());
        z.extend_from_slice(&(cd_size as u32).to_le_bytes());
        z.extend_from_slice(&(cd_start as u32).to_le_bytes());
        z.extend_from_slice(&0u16.to_le_bytes());

        let zip = ZipArchive::open(z).unwrap();
        assert_eq!(zip.entry_names(), vec!["a.txt"]);
        assert_eq!(zip.read_by_name("a.txt").unwrap().unwrap(), b"hi");
    }
}
