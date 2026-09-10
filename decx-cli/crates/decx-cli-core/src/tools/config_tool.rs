//! `config` tool — read and write the unified configuration
//! (`DECX_HOME/config.json`).

use serde_json::{json, Value};

use crate::error::{DecxError, DecxResult};
use crate::iface::{ArgSpec as A, Args, CommandSpec, Interface};
use crate::tools::ToolContext;

pub fn interface() -> Interface {
    Interface::new("config", "Read and write the unified configuration (DECX_HOME/config.json)").commands(vec![
        CommandSpec::leaf("get", "Print the configuration (or one key)", vec![A::pos_opt("key", "Key to print")], run_get),
        CommandSpec::leaf(
            "set",
            "Set one configuration key (typed: default_engine, default_format, default_port, monitor_interval_secs, open_timeout_secs)",
            vec![
                A::positional("key", "Configuration key"),
                A::pos_opt("value", "New value"),
                A::flag("unset", "unset", "Clear the key back to its built-in default"),
            ],
            run_set,
        ),
    ])
}

fn apply(config: &mut crate::config::Config, key: &str, value: &str) -> DecxResult<()> {
    match key {
        "default_engine" => config.default_engine = value.to_string(),
        "default_format" => {
            if !value.is_empty() {
                crate::output::OutputFormat::parse(value)?;
            }
            config.default_format = value.to_string();
        }
        "default_port" => config.server.default_port = crate::ports::parse_server_port(value)?,
        "monitor_interval_secs" => {
            config.session.monitor_interval_secs = parse_seconds(value)?;
        }
        "open_timeout_secs" => {
            config.session.open_timeout_secs = parse_seconds(value)?;
        }
        other => {
            return Err(DecxError::usage(format!(
                "Unknown key '{other}' (settable: default_engine, default_format, default_port, \
                 monitor_interval_secs, open_timeout_secs)"
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

fn read_key(config: &crate::config::Config, key: &str) -> DecxResult<Value> {
    Ok(match key {
        "default_engine" => json!(config.effective_engine(None)),
        "default_format" => json!(if config.default_format.is_empty() {
            "json".to_string()
        } else {
            config.default_format.clone()
        }),
        "default_port" => json!(config.server.default_port),
        "monitor_interval_secs" => json!(config.session.monitor_interval_secs),
        "open_timeout_secs" => json!(config.session.open_timeout_secs),
        other => {
            return Err(DecxError::usage(format!(
                "Unknown key '{other}' (settable: default_engine, default_format, default_port, \
                 monitor_interval_secs, open_timeout_secs)"
            )))
        }
    })
}

fn run_get(ctx: &ToolContext, a: &Args) -> DecxResult<Value> {
    let config = crate::config::Config::load(&ctx.home);
    match a.opt_str("key") {
        Some(key) => read_key(&config, key),
        None => {
            let tools: Vec<&str> = config.tools.iter().map(|t| t.name.as_str()).collect();
            Ok(json!({
                "config_version": config.config_version,
                "default_engine": config.effective_engine(None),
                "default_format": if config.default_format.is_empty() { json!("json") } else { json!(config.default_format) },
                "server": { "default_port": config.server.default_port },
                "session": {
                    "monitor_interval_secs": config.session.monitor_interval_secs,
                    "open_timeout_secs": config.session.open_timeout_secs,
                },
                "tools": tools,
            }))
        }
    }
}

fn run_set(ctx: &ToolContext, a: &Args) -> DecxResult<Value> {
    let key = a.str("key").to_string();
    let mut config = crate::config::Config::load(&ctx.home);
    let (applied, effective) = if a.flag("unset") {
        let defaults = crate::config::Config::default();
        let default_value = match key.as_str() {
            "default_port" => defaults.server.default_port.to_string(),
            "monitor_interval_secs" => defaults.session.monitor_interval_secs.to_string(),
            "open_timeout_secs" => defaults.session.open_timeout_secs.to_string(),
            _ => String::new(),
        };
        apply(&mut config, &key, &default_value)?;
        (format!("--unset {key}"), read_key(&config, &key)?)
    } else {
        let value = a.str("value").to_string();
        apply(&mut config, &key, &value)?;
        (format!("{key} = {value}"), read_key(&config, &key)?)
    };
    config.save(&ctx.home)?;
    Ok(json!({ "applied": applied, "effective": effective }))
}
