//! Structured error codes — mirrors the Kotlin `DecxError` list in
//! `decx/decx-core/src/main/kotlin/jadx/plugins/decx/api/DecxError.kt` so the
//! native server speaks the same `{ "error": "E0xx", "message": "..." }` shape.

use std::fmt;

#[derive(Debug, Clone)]
pub struct DecxError {
    pub code: String,
    pub message: String,
}

impl DecxError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
        }
    }

    pub fn class_not_found(cls: impl fmt::Display) -> Self {
        Self::new("CLASS_NOT_FOUND", format!("class not found: {cls}"))
    }

    pub fn method_not_found(mth: impl fmt::Display) -> Self {
        Self::new("METHOD_NOT_FOUND", format!("method not found: {mth}"))
    }

    pub fn field_not_found(fld: impl fmt::Display) -> Self {
        Self::new("FIELD_NOT_FOUND", format!("field not found: {fld}"))
    }

    pub fn invalid_parameter(msg: impl Into<String>) -> Self {
        Self::new("INVALID_PARAMETER", msg)
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        Self::new("INTERNAL_ERROR", msg)
    }

    pub fn interface_not_found(iface: impl fmt::Display) -> Self {
        Self::new("INTERFACE_NOT_FOUND", format!("interface not found: {iface}"))
    }

    pub fn service_impl_not_found(iface: impl fmt::Display) -> Self {
        Self::new(
            "SERVICE_IMPL_NOT_FOUND",
            format!("no implementation found for system service interface: {iface}"),
        )
    }

    pub fn manifest_not_found() -> Self {
        Self::new("MANIFEST_NOT_FOUND", "AndroidManifest.xml not found in the loaded target")
    }

    pub fn resource_not_found(res: impl fmt::Display) -> Self {
        Self::new("RESOURCE_NOT_FOUND", format!("resource not found: {res}"))
    }

    pub fn no_strings_found() -> Self {
        Self::new("NO_STRINGS_FOUND", "no string resources found")
    }

    pub fn no_main_activity() -> Self {
        Self::new("NO_MAIN_ACTIVITY", "no main (launcher) activity found in AndroidManifest.xml")
    }

    pub fn no_application() -> Self {
        Self::new("NO_APPLICATION", "no custom Application class declared in AndroidManifest.xml")
    }

    pub fn decompilation_skipped(msg: impl Into<String>) -> Self {
        Self::new("DECOMPILATION_SKIPPED", msg)
    }

    pub fn empty_search_key() -> Self {
        Self::new("EMPTY_SEARCH_KEY", "search key must not be empty")
    }

    /// HTTP status for this error, mirroring the Kotlin status mapping.
    pub fn http_status(&self) -> u16 {
        match self.code.as_str() {
            "INVALID_PARAMETER" | "EMPTY_SEARCH_KEY" => 400,
            "CLASS_NOT_FOUND" | "RESOURCE_NOT_FOUND" | "MANIFEST_NOT_FOUND" | "FIELD_NOT_FOUND"
            | "INTERFACE_NOT_FOUND" | "SERVICE_IMPL_NOT_FOUND" | "NO_STRINGS_FOUND"
            | "NO_MAIN_ACTIVITY" | "NO_APPLICATION" | "METHOD_NOT_FOUND" | "UNKNOWN_ENDPOINT" => 404,
            "SERVICE_ERROR" | "DECOMPILATION_SKIPPED" | "NOT_GUI_MODE" => 503,
            "REQUEST_TIMEOUT" => 504,
            _ => 500,
        }
    }
}

impl fmt::Display for DecxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for DecxError {}

pub type Result<T> = std::result::Result<T, DecxError>;
