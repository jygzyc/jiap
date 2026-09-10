//! `settings` command group — read and write the CLI's runtime settings
//! (`DECX_HOME/settings.json`).
//!
//! Distinct from the compile-time engine registry (`config.json`, baked into
//! the binary at build time): settings are user-tunable runtime values.

use serde_json::{json, Value};

use crate::commands::{Args, ToolContext};
use crate::error::{DecxError, DecxResult};
use crate::iface::{ArgSpec as A, CommandSpec, Interface};
use crate::settings::Settings;

pub fn interface() -> Interface {
    Interface::new("settings", "Read and write CLI runtime settings (DECX_HOME/settings.json)").commands(vec![
        CommandSpec::leaf(
            "get",
            "Print the settings (or one key)",
            vec![A::pos_opt("key", "Key to print")],
            run_get,
        ),
        CommandSpec::leaf(
            "set",
            "Set one settings key (default_engine, default_format, default_port, monitor_interval_secs, open_timeout_secs)",
            vec![
                A::positional("key", "Settings key"),
                A::pos_opt("value", "New value"),
                A::flag("unset", "unset", "Clear the key back to its built-in default"),
            ],
            run_set,
        ),
    ])
}

const KEYS: &str = "default_engine, default_format, default_port, monitor_interval_secs, open_timeout_secs";

fn apply(settings: &mut Settings, key: &str, value: &str) -> DecxResult<()> {
    match key {
        "default_engine" => {
            if !crate::engine::EngineCatalog::global().get(value).is_some() {
                return Err(DecxError::usage(format!(
                    "Unknown engine '{value}' (see: decx engine list)"
                )));
            }
            settings.default_engine = value.to_string();
        }
        "default_format" => {
            if !value.is_empty() {
                crate::output::OutputFormat::parse(value)?;
            }
            settings.default_format = value.to_string();
        }
        "default_port" => settings.server.default_port = crate::ports::parse_server_port(value)?,
        "monitor_interval_secs" => {
            settings.session.monitor_interval_secs = parse_seconds(value)?;
        }
        "open_timeout_secs" => {
            settings.session.open_timeout_secs = parse_seconds(value)?;
        }
        other => {
            return Err(DecxError::usage(format!(
                "Unknown key '{other}' (settable: {KEYS})"
            )))
        }
    }
    Ok(())
}

fn parse_seconds(value: &str) -> DecxResult<u64> {
    value
        .parse()
        .map_err(|_| DecxError::usage(format!("'{value}' is not a valid number of seconds")))
}

fn read_key(settings: &Settings, key: &str) -> DecxResult<Value> {
    Ok(match key {
        "default_engine" => json!(settings.effective_engine(None)),
        "default_format" => json!(if settings.default_format.is_empty() {
            "json".to_string()
        } else {
            settings.default_format.clone()
        }),
        "default_port" => json!(settings.server.default_port),
        "monitor_interval_secs" => json!(settings.session.monitor_interval_secs),
        "open_timeout_secs" => json!(settings.session.open_timeout_secs),
        other => {
            return Err(DecxError::usage(format!(
                "Unknown key '{other}' (settable: {KEYS})"
            )))
        }
    })
}

fn run_get(ctx: &ToolContext, a: &Args) -> DecxResult<Value> {
    let settings = Settings::load(&ctx.home);
    match a.opt_str("key") {
        Some(key) => read_key(&settings, key),
        None => Ok(json!({
            "settings_version": settings.settings_version,
            "default_engine": settings.effective_engine(None),
            "default_format": if settings.default_format.is_empty() { json!("json") } else { json!(settings.default_format) },
            "server": { "default_port": settings.server.default_port },
            "session": {
                "monitor_interval_secs": settings.session.monitor_interval_secs,
                "open_timeout_secs": settings.session.open_timeout_secs,
            },
            "external_tools": settings.tools.len(),
        })),
    }
}

fn run_set(ctx: &ToolContext, a: &Args) -> DecxResult<Value> {
    let key = a.str("key")?.to_string();
    let mut settings = Settings::load(&ctx.home);
    let (applied, effective) = if a.flag("unset") {
        let defaults = Settings::default();
        let default_value = match key.as_str() {
            "default_port" => defaults.server.default_port.to_string(),
            "monitor_interval_secs" => defaults.session.monitor_interval_secs.to_string(),
            "open_timeout_secs" => defaults.session.open_timeout_secs.to_string(),
            _ => String::new(),
        };
        apply(&mut settings, &key, &default_value)?;
        (format!("--unset {key}"), read_key(&settings, &key)?)
    } else {
        let value = a.str("value")?.to_string();
        apply(&mut settings, &key, &value)?;
        (format!("{key} = {value}"), read_key(&settings, &key)?)
    };
    settings.save(&ctx.home)?;
    Ok(json!({ "applied": applied, "effective": effective }))
}
