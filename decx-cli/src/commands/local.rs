//! Registry of LOCAL tool-domain commands.
//!
//! A config.json leaf with `"local": "<id>"` is implemented in-process by
//! a Rust handler instead of POSTing to an engine server (e.g. adb device
//! inspection, which never touches a server). The id is the dotted command
//! path; `build.rs` compiles it into the static registry and this table
//! binds it to the implementation. An id present in config.json but
//! missing here panics at startup (`iface::from_static`) — the config is
//! compile-time validated, so this is a programming-error tripwire, not a
//! user-facing path.

use crate::iface::Handler;

pub fn lookup(id: &str) -> Option<Handler> {
    match id {
        "java.android.device.system-services" => {
            Some(crate::android_sdk::device_system_services)
        }
        "java.android.device.permission-info" => {
            Some(crate::android_sdk::device_permission_info)
        }
        "java.android.framework.collect" => Some(crate::android_sdk::framework::framework_collect),
        "java.android.framework.process" => Some(crate::android_sdk::framework::framework_process),
        "java.android.framework.pack" => Some(crate::android_sdk::framework::framework_pack),
        "java.android.framework.run" => Some(crate::android_sdk::framework::framework_run),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_local_id_in_config_is_registered() {
        // Walk the compiled-in tool registry and make sure no local leaf
        // references an unregistered handler (from_static would panic).
        for tool in crate::engines_gen::TOOLS {
            fn walk(cmd: &'static crate::spec::CmdSpec) {
                if cmd.subs.is_empty() {
                    if let Some(id) = cmd.local {
                        assert!(
                            lookup(id).is_some(),
                            "local handler '{id}' is not registered"
                        );
                    }
                } else {
                    for sub in cmd.subs {
                        walk(sub);
                    }
                }
            }
            for cmd in tool.commands {
                walk(cmd);
            }
        }
    }

    #[test]
    fn unknown_id_returns_none() {
        assert!(lookup("no.such.handler").is_none());
    }
}
