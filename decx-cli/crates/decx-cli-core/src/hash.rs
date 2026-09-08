//! File hashing (sha256) used for project identity and session reuse.

use std::io::Read;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::error::{DecxError, DecxResult};

/// Stream a file through sha256 and return the lowercase hex digest.
pub fn hash_file(path: &Path) -> DecxResult<String> {
    let mut file = std::fs::File::open(path)
        .map_err(|e| DecxError::file(format!("cannot open {}: {e}", path.display()), Some(path.display().to_string())))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|e| DecxError::file(format!("cannot read {}: {e}", path.display()), Some(path.display().to_string())))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streaming_hash_matches_oneshot() {
        let tmp = std::env::temp_dir().join(format!("decx-hash-test-{}", std::process::id()));
        let content = vec![0xABu8; 200 * 1024]; // spans several read chunks
        std::fs::write(&tmp, &content).unwrap();
        let digest = hash_file(&tmp).unwrap();
        assert_eq!(digest.len(), 64);
        assert_eq!(digest, sha256_hex(&content));
        let _ = std::fs::remove_file(&tmp);
    }

    fn sha256_hex(content: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content);
        format!("{:x}", hasher.finalize())
    }
}
