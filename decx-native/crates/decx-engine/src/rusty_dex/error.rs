// decx-engine rusty_dex module: DEX parser (decx-native).
// Adapted from rusty-rs/rusty-dex 0.2.0 (Apache-2.0): https://github.com/rusty-rs/rusty-dex
// Baseline: the copy bundled in the asLody/dexdec v1.0.2 workspace.
// MODIFIED for decx-native: zip I/O rerouted to decx-apk; thiserror /
// regex / lazy_static / byteorder replaced by hand-written std code.
// Provenance and modification policy: crates/decx-engine/VENDORED.md
//! Collection of error types for DEX files parsing
//!
//! (decx-native: `thiserror` derive replaced by manual impls;
//! `zip::result::ZipError` replaced by a String payload.)

use std::fmt;

/// All errors that can be returned by the parser
#[derive(Debug)]
pub enum DexError {
    /// The input archive could not be opened or decoded.
    InvalidArchive(String),
    /// The checksum of the header does not match the one in the DEX header
    InvalidChecksumError,
    /// The header of the file is too short to be a valid DEX header
    DexHeaderTooShortError,
    /// The endianness tag of the header is invalid
    InvalidEndianessTag,
    /// The stream ended abruptly
    NoDataLeftError,
    /// The unsigned LEB128 value has more than 5 bytes
    InvalidUleb128Value,
    /// The signed LEB128 value has more than 5 bytes
    InvalidUleb128p1Value,
    /// The unsigned LEB128p1 value has more than 5 bytes
    InvalidSleb128Value,
    /// Attempted to move the cursor after the end of the stream
    SeekError(std::io::Error),
    /// Requested type index is not in the list of types
    InvalidTypeIdx,
    /// Requested string index is not in the list of strings
    InvalidStringIdx,
    /// A string_data_item is not valid Modified UTF-8.
    InvalidMutf8(&'static str),
    /// A string_data_item length does not match its declared UTF-16 size.
    InvalidStringLength { expected: u32, actual: usize },
    /// Requested field index is not in the list of fields
    InvalidFieldIdx,
    /// Requested method index is not in the list of methods
    InvalidMethodIdx,
    /// Requested class definition index is not in the list of classes
    InvalidClassIdx,
    /// Encountered an invalid or unused opcode
    InvalidOpCode,
    /// Encountered an invalid encoded value
    InvalidEncodedValue,
}

impl fmt::Display for DexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DexError::InvalidArchive(msg) => write!(f, "invalid DEX archive: {msg}"),
            DexError::InvalidChecksumError => write!(f, "computed checksum does not match one in header"),
            DexError::DexHeaderTooShortError => write!(f, "DEX header too short"),
            DexError::InvalidEndianessTag => write!(f, "invalid endianness tag"),
            DexError::NoDataLeftError => write!(f, "no data left to read"),
            DexError::InvalidUleb128Value => write!(f, "too many bytes in unsigned LEB128 value"),
            DexError::InvalidUleb128p1Value => write!(f, "too many bytes in unsigned LEB128p1 value"),
            DexError::InvalidSleb128Value => write!(f, "too many bytes in signed LEB128 value"),
            DexError::SeekError(e) => write!(f, "cannot move reader to requested offset: {e}"),
            DexError::InvalidTypeIdx => write!(f, "cannot find element in types list"),
            DexError::InvalidStringIdx => write!(f, "cannot find element in strings list"),
            DexError::InvalidMutf8(msg) => write!(f, "invalid MUTF-8 string: {msg}"),
            DexError::InvalidStringLength { expected, actual } => {
                write!(f, "MUTF-8 string has {actual} UTF-16 code units, expected {expected}")
            }
            DexError::InvalidFieldIdx => write!(f, "cannot find element in fields list"),
            DexError::InvalidMethodIdx => write!(f, "cannot find element in methods list"),
            DexError::InvalidClassIdx => write!(f, "cannot find element in class definitions list"),
            DexError::InvalidOpCode => write!(f, "cannot parse instruction opcode"),
            DexError::InvalidEncodedValue => write!(f, "cannot parse encoded value"),
        }
    }
}

impl std::error::Error for DexError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            DexError::SeekError(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for DexError {
    fn from(e: std::io::Error) -> Self {
        DexError::SeekError(e)
    }
}
