//! Structured errors with sysexits-style exit codes.
//!
//! Following the opencli convention, every CLI failure maps to a predictable
//! exit code class (see `sysexits.h`) so shell wrappers and AI agents can
//! branch on failure kinds instead of parsing stderr:
//!
//! - `0`  success
//! - `64` `EX_USAGE` — bad arguments / invalid parameters
//! - `66` `EX_NOINPUT` — missing input file or named session/project
//! - `69` `EX_UNAVAILABLE` — server / dependency unavailable (connection refused)
//! - `70` `EX_SOFTWARE` — internal error
//! - `75` `EX_TEMPFAIL` — timeouts (health wait, request timeout)

use std::fmt;

use serde_json::{json, Value};

pub const EX_OK: i32 = 0;
pub const EX_USAGE: i32 = 64;
pub const EX_NOINPUT: i32 = 66;
pub const EX_UNAVAILABLE: i32 = 69;
pub const EX_SOFTWARE: i32 = 70;
pub const EX_TEMPFAIL: i32 = 75;

/// A structured CLI error: machine-readable `code`, human message, optional
/// details, and the sysexits exit code the process should exit with.
#[derive(Debug, Clone)]
pub struct DecxError {
    pub code: String,
    pub message: String,
    pub details: Value,
    pub exit_code: i32,
}

impl DecxError {
    pub fn new(code: impl Into<String>, message: impl Into<String>, exit_code: i32) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: Value::Null,
            exit_code,
        }
    }

    pub fn with_details(mut self, details: Value) -> Self {
        self.details = details;
        self
    }

    /// Bad arguments or invalid parameter values. `EX_USAGE` (64).
    pub fn usage(message: impl Into<String>) -> Self {
        Self::new("USAGE_ERROR", message, EX_USAGE)
    }

    /// A referenced file does not exist. `EX_NOINPUT` (66).
    pub fn file(message: impl Into<String>, path: Option<String>) -> Self {
        let mut err = Self::new("FILE_ERROR", message, EX_NOINPUT);
        if let Some(p) = path {
            err.details = json!({ "filePath": p });
        }
        err
    }

    /// A referenced session/project or server resource was not found. `EX_NOINPUT` (66).
    pub fn not_found(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(code, message, EX_NOINPUT)
    }

    /// Process lifecycle failures (spawn, kill). `EX_SOFTWARE` (70).
    pub fn process(message: impl Into<String>) -> Self {
        Self::new("PROCESS_ERROR", message, EX_SOFTWARE)
    }

    /// Invalid port value. `EX_USAGE` (64).
    pub fn invalid_port(value: impl fmt::Display) -> Self {
        Self::new("PROCESS_ERROR", format!("Invalid port: {value}"), EX_USAGE)
    }

    /// HTTP request to the DECX server timed out. `EX_TEMPFAIL` (75).
    pub fn timeout(message: impl Into<String>) -> Self {
        Self::new("TIMEOUT", message, EX_TEMPFAIL)
    }

    /// The DECX server could not be reached. `EX_UNAVAILABLE` (69).
    pub fn connection(message: impl Into<String>) -> Self {
        Self::new("CONNECTION_ERROR", message, EX_UNAVAILABLE)
    }

    /// Server-side error response. `EX_UNAVAILABLE` (69) unless overridden.
    pub fn server(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(code, message, EX_UNAVAILABLE)
    }

    /// A feature that exists in the TypeScript CLI but is not ported yet.
    pub fn not_ported(what: impl fmt::Display) -> Self {
        Self::new(
            "NOT_PORTED",
            format!(
                "{what} is not ported to the Rust CLI yet; use the TypeScript decx-cli for this command."
            ),
            EX_SOFTWARE,
        )
    }

    /// Catch-all internal error. `EX_SOFTWARE` (70).
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new("INTERNAL_ERROR", message, EX_SOFTWARE)
    }

    /// Sentinel for `decx tools run`: the child process already ran to
    /// completion with inherited stdio, so the CLI must exit with its code
    /// without printing anything.
    pub fn passthrough_exit(code: i32) -> Self {
        Self::new("__PASSTHROUGH_EXIT__", String::new(), code)
    }

    pub fn is_passthrough_exit(&self) -> bool {
        self.code == "__PASSTHROUGH_EXIT__"
    }

    /// Render as a structured JSON object (for `--format json` stderr logs).
    pub fn to_json(&self) -> Value {
        let mut obj = json!({
            "error": self.code,
            "message": self.message,
            "exitCode": self.exit_code,
        });
        if let Value::Object(map) = &self.details {
            for (k, v) in map {
                obj[k.clone()] = v.clone();
            }
        }
        obj
    }
}

impl fmt::Display for DecxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

impl std::error::Error for DecxError {}

pub type DecxResult<T> = Result<T, DecxError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_codes_follow_sysexits() {
        assert_eq!(DecxError::usage("x").exit_code, 64);
        assert_eq!(DecxError::file("x", None).exit_code, 66);
        assert_eq!(DecxError::not_found("SESSION_NOT_FOUND", "x").exit_code, 66);
        assert_eq!(DecxError::connection("x").exit_code, 69);
        assert_eq!(DecxError::process("x").exit_code, 70);
        assert_eq!(DecxError::timeout("x").exit_code, 75);
    }

    #[test]
    fn to_json_merges_details() {
        let err = DecxError::file("nope", Some("a.apk".into()));
        let v = err.to_json();
        assert_eq!(v["error"], "FILE_ERROR");
        assert_eq!(v["filePath"], "a.apk");
        assert_eq!(v["exitCode"], 66);
    }
}
