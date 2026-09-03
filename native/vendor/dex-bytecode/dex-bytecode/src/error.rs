use alloc::string::String;
use core::fmt;

use std::error::Error;

#[derive(Clone)]
pub struct DexError {
    error: String,
}

impl DexError {
    /// Invalid instruction or truncated buffer.
    pub fn invalid(msg: impl Into<String>) -> Self {
        Self { error: msg.into() }
    }

    /// Invalid instruction with dynamic message (allocates).
    pub fn invalid_owned(msg: String) -> Self {
        Self { error: msg }
    }
}

impl core::fmt::Debug for DexError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DexError")
            .field("error", &self.error)
            .finish()
    }
}

impl Error for DexError {}

impl fmt::Display for DexError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.error)
    }
}
