//! Minimal `log` facade for decx-engine (original decx-native code).
//! Macros no-op unless `DECX_DEXDEC_LOG` is set, then print to stderr.
//! Folded in from the former standalone `log` shim crate; vendored upstream
//! files keep their `use log::...` shape via the module re-export below.

#[macro_export]
macro_rules! log_emit {
    ($lvl:expr, $($arg:tt)*) => {{
        if std::env::var_os("DECX_DEXDEC_LOG").is_some() {
            eprintln!("[{}] {}", $lvl, format!($($arg)*));
        }
    }};
}

#[macro_export]
macro_rules! error { ($($arg:tt)*) => { $crate::log_emit!("ERROR", $($arg)*) }; }

#[macro_export]
macro_rules! warn { ($($arg:tt)*) => { $crate::log_emit!("WARN", $($arg)*) }; }

#[macro_export]
macro_rules! info { ($($arg:tt)*) => { $crate::log_emit!("INFO", $($arg)*) }; }

#[macro_export]
macro_rules! debug { ($($arg:tt)*) => { $crate::log_emit!("DEBUG", $($arg)*) }; }

#[macro_export]
macro_rules! trace { ($($arg:tt)*) => { $crate::log_emit!("TRACE", $($arg)*) }; }

// decx-native: `#[macro_export]` macros live at the crate root; re-bind them
// under this module's path so `use crate::log::info;` resolves everywhere.
#[allow(unused_imports)]
pub use crate::{debug, error, info, log_emit, trace, warn};
