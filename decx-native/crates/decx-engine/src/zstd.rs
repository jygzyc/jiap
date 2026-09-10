//! Identity `zstd` (store raw) for decx-engine (original decx-native code).
//! Only the surface codec.rs uses: `zstd::stream::{encode_all, decode_all}`.

pub mod stream {
    use std::io::Read;

    /// Identity: reads all bytes (level ignored).
    pub fn encode_all<R: Read>(src: R, _level: i32) -> std::io::Result<Vec<u8>> {
        let mut out = Vec::new();
        let mut src = src;
        src.read_to_end(&mut out)?;
        Ok(out)
    }

    /// Identity: reads all bytes.
    pub fn decode_all<R: Read>(src: R) -> std::io::Result<Vec<u8>> {
        let mut out = Vec::new();
        let mut src = src;
        src.read_to_end(&mut out)?;
        Ok(out)
    }
}

pub mod stream_read {
    // placeholder for stream::read users (none in vendored code today)
}
