//! `config` tool — read and write the unified configuration
//! (`DECX_HOME/config.json`). One file covers CLI defaults, session
//! defaults, the server-jar record, and the external-tool registry.

use clap::{Arg, ArgAction, ArgMatches, Command};
use serde_json::{json, Value};

use crate::error::{DecxError, DecxResult};

use super::{Tool, ToolContext};

pub struct ConfigTool;

fn command() -> Command {
    Command::new("config")
        .about("Read and write the unified configuration (DECX_HOME/config.json)")
        .long_about(
            "One file backs every decx setting: CLI defaults (default_engine, default_format), \
             the server port, session defaults (monitor_interval_secs, open_timeout_secs), the \
             installed server-jar version, and the external-tool registry. \
             `set` knows the typed keys: default_engine, default_format, default_port, \
             monitor_interval_secs, open_timeout_secs.",
        )
        .subcommands([
            Command::new("get")
                .about("Print the full unified configuration")
                .arg(Arg::new("key").num_args(0..=1).value_name("KEY").help("Print one key's value instead")),
            Command::new("set")
                .about("Set one configuration key")
                .arg(Arg::new("key").required(true).value_name("KEY").value_parser([
                    "default_engine",
                    "default_format",
                    "default_port",
                    "monitor_interval_secs",
                    "open_timeout_secs",
                ]))
                .arg(Arg::new("value").value_name("VALUE").required_unless_present("unset").num_args(0..=1))
                .arg(
                    Arg::new("unset")
                        .long("unset")
                        .action(ArgAction::SetTrue)
                        .help("Clear the key back to its built-in default (VALUE ignored)"),
                ),
        ])
}

const SETTABLE: [&str; 5] = [
    "default_engine",
    "default_format",
    "default_port",
    "monitor_interval_secs",
    "open_timeout_secs",
];

impl ConfigTool {
    fn apply_set(config: &mut crate::config::Config, key: &str, value: &str) -> DecxResult<Value> {
        let typed: Result<(), DecxError> = match key {
            "default_port" => {
                config.server.default_port = crate::ports::parse_server_port(value)?;
                Ok(())
            }
            "monitor_interval_secs" => {
                config.session.monitor_interval_secs = value.parse().map_err(|_| {
                    DecxError::usage(format!("'{value}' is not a valid number of seconds"))
                })?;
                Ok(())
            }
            "open_timeout_secs" => {
                config.session.open_timeout_secs = value.parse().map_err(|_| {
                    DecxError::usage(format!("'{value}' is not a valid number of seconds"))
                })?;
                Ok(())
            }
            "default_engine" => {
                config.default_engine = value.to_string();
                Ok(())
            }
            "default_format" => {
                if !value.is_empty() {
                    crate::output::OutputFormat::parse(value)?;
                }
                config.default_format = value.to_string();
                Ok(())
            }
            _ => Err(DecxError::usage(format!(
                "Unknown key '{key}' (settable: {})",
                SETTABLE.join(", ")
            ))),
        };
        typed?;
        Ok(json!({ "set": key, "value": value }))
    }

    fn read_key(config: &crate::config::Config, key: &str) -> DecxResult<Value> {
        match key {
            "default_engine" => Ok(json!(config.effective_engine(None))),
            "default_format" => Ok(json!(if config.default_format.is_empty() {
                "json".to_string()
            } else {
                config.default_format.clone()
            })),
            "default_port" => Ok(json!(config.server.default_port)),
            "monitor_interval_secs" => Ok(json!(config.session.monitor_interval_secs)),
            "open_timeout_secs" => Ok(json!(config.session.open_timeout_secs)),
            other => Err(DecxError::usage(format!(
                "Unknown key '{other}' (settable: {})",
                SETTABLE.join(", ")
            ))),
        }
    }
}

impl Tool for ConfigTool {
    fn id(&self) -> &'static str {
        "config"
    }

    fn commands(&self) -> Vec<Command> {
        vec![command()]
    }

    fn run(&self, ctx: &ToolContext, matches: &ArgMatches) -> DecxResult<Value> {
        let Some((name, m)) = matches.subcommand() else {
            return Err(DecxError::usage("No config subcommand given (get | set)"));
        };
        let mut config = crate::config::Config::load(&ctx.home);
        match name {
            "get" => match m.get_one::<String>("key").map(String::as_str).filter(|s| !s.is_empty()) {
                Some(key) => Self::read_key(&config, key),
                None => {
                    let mut summary = json!({
                        "config_version": config.config_version,
                        "default_engine": config.effective_engine(None),
                        "default_format": if config.default_format.is_empty() { json!("json") } else { json!(config.default_format) },
                        "server": { "default_port": config.server.default_port },
                        "session": {
                            "monitor_interval_secs": config.session.monitor_interval_secs,
                            "open_timeout_secs": config.session.open_timeout_secs,
                        },
                        "tools": config.tools.iter().map(|t| t.name.clone()).collect::<Vec<_>>(),
                    });
                    summary["raw"] = serde_json::to_value(&config).unwrap_or(json!({}));
                    Ok(summary)
                }
            },
            "set" => {
                let key = m.get_one::<String>("key").map(String::as_str).unwrap_or_default();
                if m.get_flag("unset") {
                    let defaults = crate::config::Config::default();
                    let default_value = match key {
                        "default_port" => defaults.server.default_port.to_string(),
                        "monitor_interval_secs" => defaults.session.monitor_interval_secs.to_string(),
                        "open_timeout_secs" => defaults.session.open_timeout_secs.to_string(),
                        _ => String::new(),
                    };
                    let value = Self::apply_set(&mut config, key, &default_value)?;
                    config.save(&ctx.home)?;
                    Ok(json!({ "unset": key, "value": value["value"] }))
                } else {
                    let value = m.get_one::<String>("value").map(String::as_str).unwrap_or_default();
                    let result = Self::apply_set(&mut config, key, value)?;
                    config.save(&ctx.home)?;
                    Ok(result)
                }
            }
            other => Err(DecxError::usage(format!("Unknown config subcommand '{other}'"))),
        }
    }
}


