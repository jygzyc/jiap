//! Android capability SDK, imported by the CLI.
//!
//! This module is a capability layer, not CLI plumbing: everything here
//! runs locally (adb spawns, parsing) and never touches engine servers.
//! The device-inspection commands are declared in `config.json` under the
//! java tool domain as LOCAL commands (`decx java android device ...`);
//! this module binds their handler ids (see `commands::local`) to the Rust
//! implementations below.
//!
//! What deliberately does NOT exist anymore:
//! - a top-level `decx android` command group (cancelled — java takes over)
//! - `framework open` — removed. A framework jar is opened like any other
//!   java-domain target: `decx session open <framework.jar>`. Collection and
//!   processing live in `framework` (collect/process/pack/run) below.

pub mod adb;
pub mod erofs;
pub mod ext4;
pub mod framework;
pub mod zip_util;

use serde_json::Value;

use crate::commands::{Args, ToolContext};
use crate::error::DecxResult;

/// `decx java android device system-services` — list live Binder/system
/// services from the connected device.
pub fn device_system_services(_ctx: &ToolContext, a: &Args) -> DecxResult<Value> {
    let mut client = adb_client(a);
    client.ensure_available()?;
    let (total, services) = client.list_system_services()?;
    Ok(adb::filter_system_services(
        total,
        &services,
        a.opt_str("grep"),
    ))
}

/// `decx java android device permission-info` — structured metadata for
/// one Android permission.
pub fn device_permission_info(_ctx: &ToolContext, a: &Args) -> DecxResult<Value> {
    let mut client = adb_client(a);
    client.ensure_available()?;
    client.permission_info(a.str("permission")?)
}

fn adb_client(a: &Args) -> adb::AdbClient {
    adb::AdbClient::new(
        a.opt_str("adb-path").map(str::to_string),
        a.opt_str("serial").map(str::to_string),
    )
}
