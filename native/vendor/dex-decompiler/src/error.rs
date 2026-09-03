//! Error types for DEX parsing and decompilation.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum DexDecompilerError {
    #[error("DEX parse error: {0}")]
    Parse(String),

    #[error("Invalid magic or unsupported DEX version")]
    InvalidMagic,

    #[error("Truncated or out of bounds: {0}")]
    Truncated(String),

    #[error("Disassembly error: {0}")]
    Disassembly(String),

    #[error("Decompilation error: {0}")]
    Decompilation(String),
}

pub type Result<T> = std::result::Result<T, DexDecompilerError>;
