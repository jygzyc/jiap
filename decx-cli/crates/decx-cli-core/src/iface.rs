//! The standard interface protocol between decx (tool host) and decx-cli
//! (unified command line).
//!
//! A tool never writes CLI plumbing. It declares an [`Interface`] — pure
//! data: command names, argument descriptors, help text — and one
//! [`Handler`] per leaf command. decx-cli owns a single generic engine that
//! compiles any set of interfaces into a clap tree and dispatches parsed
//! arguments back to the declared handlers.
//!
//! ```text
//! tool side (decx)                     cli side (decx-cli)
//! ────────────────                     ───────────────────
//! Interface { commands, args }  ─────► build_cli(&[Interface])  → clap tree
//! Handler(&ToolCtx, &Args)      ◄───── dispatch(root_matches) → leaf handler
//! ```

use std::collections::HashMap;

use serde_json::Value;

use crate::error::{DecxError, DecxResult};
use crate::tools::ToolContext;

/// How an argument is parsed and presented.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgKind {
    /// Boolean switch (`--force`).
    Flag,
    /// Single value (`--port 1234`).
    Value,
    /// Repeatable value (`--script a --script b`) → Vec<String>.
    Multi,
    /// Required positional (`<FILE>`); `id` is the value name.
    Positional,
    /// Everything after the first positional is collected verbatim
    /// (jadx passthrough); must be the last argument.
    Trailing,
}

/// One argument descriptor.
#[derive(Debug, Clone, Copy)]
pub struct ArgSpec {
    pub id: &'static str,
    /// `--<long>`; ignored for [`ArgKind::Positional`] / `Trailing`.
    pub long: &'static str,
    pub kind: ArgKind,
    pub required: bool,
    /// Allowed values (enum validation); empty = free-form.
    pub values: &'static [&'static str],
    pub help: &'static str,
}

impl ArgSpec {
    pub fn positional(id: &'static str, help: &'static str) -> Self {
        Self {
            id,
            long: "",
            kind: ArgKind::Positional,
            required: true,
            values: &[],
            help,
        }
    }

    /// Optional positional (`[NAME]`).
    pub fn pos_opt(id: &'static str, help: &'static str) -> Self {
        Self {
            required: false,
            ..Self::positional(id, help)
        }
    }

    pub fn opt(id: &'static str, long: &'static str, help: &'static str) -> Self {
        Self {
            id,
            long,
            kind: ArgKind::Value,
            required: false,
            values: &[],
            help,
        }
    }

    pub fn required_opt(id: &'static str, long: &'static str, help: &'static str) -> Self {
        Self {
            required: true,
            ..Self::opt(id, long, help)
        }
    }

    pub fn flag(id: &'static str, long: &'static str, help: &'static str) -> Self {
        Self {
            id,
            long,
            kind: ArgKind::Flag,
            required: false,
            values: &[],
            help,
        }
    }

    pub fn multi(id: &'static str, long: &'static str, help: &'static str) -> Self {
        Self {
            id,
            long,
            kind: ArgKind::Multi,
            required: false,
            values: &[],
            help,
        }
    }

    pub fn trailing(id: &'static str, help: &'static str) -> Self {
        Self {
            id,
            long: "",
            kind: ArgKind::Trailing,
            required: false,
            values: &[],
            help,
        }
    }

    /// Restrict to an enum of values (still optional unless
    /// [`ArgSpec::required_opt`] is used instead).
    pub fn one_of(id: &'static str, long: &'static str, values: &'static [&'static str], help: &'static str) -> Self {
        Self {
            values,
            ..Self::opt(id, long, help)
        }
    }
}

/// One command (leaf: has a handler; group: has subcommands).
#[derive(Clone)]
pub struct CommandSpec {
    pub name: &'static str,
    pub about: &'static str,
    pub args: Vec<ArgSpec>,
    pub subcommands: Vec<CommandSpec>,
    pub handler: Option<Handler>,
}

pub type Handler = fn(&ToolContext, &Args) -> DecxResult<Value>;

impl CommandSpec {
    pub fn leaf(name: &'static str, about: &'static str, args: Vec<ArgSpec>, handler: Handler) -> Self {
        Self {
            name,
            about,
            args,
            subcommands: Vec::new(),
            handler: Some(handler),
        }
    }

    pub fn group(name: &'static str, about: &'static str, subcommands: Vec<CommandSpec>) -> Self {
        Self {
            name,
            about,
            args: Vec::new(),
            subcommands,
            handler: None,
        }
    }
}

/// The interface a tool exposes to decx-cli.
pub struct Interface {
    pub tool: &'static str,
    pub description: &'static str,
    /// Arguments shared by every leaf of this tool (e.g. the analysis
    /// target selectors `--session` / `--port`).
    pub common_args: Vec<ArgSpec>,
    pub commands: Vec<CommandSpec>,
}

impl Interface {
    pub fn new(tool: &'static str, description: &'static str) -> Self {
        Self {
            tool,
            description,
            common_args: Vec::new(),
            commands: Vec::new(),
        }
    }

    pub fn common(mut self, args: Vec<ArgSpec>) -> Self {
        self.common_args = args;
        self
    }

    pub fn commands(mut self, commands: Vec<CommandSpec>) -> Self {
        self.commands = commands;
        self
    }

    /// Materialize inheritance: push `common_args` and every group's args
    /// down into each leaf command's own args, and wrap the declared
    /// commands in one implicit root spec named after the tool. Call once
    /// before building or dispatching.
    pub fn materialize(&self) -> Interface {
        fn pushdown(spec: &CommandSpec, inherited: Vec<ArgSpec>) -> CommandSpec {
            let own: Vec<ArgSpec> = inherited.iter().chain(spec.args.iter()).copied().collect();
            if spec.subcommands.is_empty() {
                CommandSpec {
                    args: own,
                    subcommands: Vec::new(),
                    ..spec.clone()
                }
            } else {
                CommandSpec {
                    subcommands: spec
                        .subcommands
                        .iter()
                        .map(|sub| pushdown(sub, own.clone()))
                        .collect(),
                    ..spec.clone()
                }
            }
        }
        let root = CommandSpec {
            subcommands: self
                .commands
                .iter()
                .map(|c| pushdown(c, self.common_args.clone()))
                .collect(),
            ..CommandSpec::group(self.tool, self.description, Vec::new())
        };
        Interface {
            tool: self.tool,
            description: self.description,
            common_args: Vec::new(),
            commands: vec![root],
        }
    }
}

/// Parsed arguments handed to a handler: every declared arg is present,
/// flags default to false, multis to empty.
#[derive(Default)]
pub struct Args {
    strings: HashMap<&'static str, String>,
    multis: HashMap<&'static str, Vec<String>>,
    flags: HashMap<&'static str, bool>,
}

impl Args {
    pub fn str(&self, id: &str) -> &str {
        self.strings.get(id).map(String::as_str).unwrap_or_default()
    }

    pub fn opt_str(&self, id: &str) -> Option<&str> {
        self.strings.get(id).map(String::as_str).filter(|s| !s.is_empty())
    }

    pub fn u64(&self, id: &str) -> Option<u64> {
        self.str(id).parse().ok()
    }

    pub fn strs(&self, id: &str) -> &[String] {
        self.multis.get(id).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn flag(&self, id: &str) -> bool {
        self.flags.get(id).copied().unwrap_or(false)
    }

    pub(crate) fn insert_string(&mut self, id: &'static str, value: String) {
        self.strings.insert(id, value);
    }

    pub(crate) fn insert_multi(&mut self, id: &'static str, values: Vec<String>) {
        self.multis.insert(id, values);
    }

    pub(crate) fn insert_flag(&mut self, id: &'static str, value: bool) {
        self.flags.insert(id, value);
    }
}

/// Walk a declared command tree alongside parsed matches and run the leaf
/// handler. Generic — this is the whole dispatch logic of decx-cli.
pub fn run_command(
    spec: &CommandSpec,
    common: &[ArgSpec],
    matches: &clap::ArgMatches,
    ctx: &ToolContext,
) -> DecxResult<Value> {
    if !spec.subcommands.is_empty() {
        let Some((name, sub)) = matches.subcommand() else {
            return Err(DecxError::usage(format!(
                "No subcommand given for '{}' (one of: {})",
                spec.name,
                spec.subcommands
                    .iter()
                    .map(|c| c.name)
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        };
        let sub_spec = spec
            .subcommands
            .iter()
            .find(|c| c.name == name)
            .ok_or_else(|| DecxError::usage(format!("Unknown subcommand '{name}'")))?;
        return run_command(sub_spec, common, sub, ctx);
    }
    let handler = spec.handler.ok_or_else(|| {
        DecxError::usage(format!("'{spec_name}' has no handler", spec_name = spec.name))
    })?;
    let args = collect_args(spec, common, matches)?;
    handler(ctx, &args)
}

/// Translate parsed matches into [`Args`] following the declaration.
fn collect_args(spec: &CommandSpec, common: &[ArgSpec], matches: &clap::ArgMatches) -> DecxResult<Args> {
    let mut args = Args::default();
    for arg in spec.args.iter().chain(common.iter()) {
        match arg.kind {
            ArgKind::Flag => args.insert_flag(arg.id, matches.get_flag(arg.id)),
            ArgKind::Value | ArgKind::Positional => {
                let value = matches
                    .get_one::<String>(arg.id)
                    .cloned()
                    .unwrap_or_default();
                if arg.required && value.is_empty() {
                    return Err(DecxError::usage(format!("--{} is required", arg.long)));
                }
                if !arg.values.is_empty() && !value.is_empty() && !arg.values.contains(&value.as_str()) {
                    return Err(DecxError::usage(format!(
                        "Invalid value '{value}' for --{} (one of: {})",
                        arg.long,
                        arg.values.join(", ")
                    )));
                }
                args.insert_string(arg.id, value);
            }
            ArgKind::Multi => args.insert_multi(
                arg.id,
                matches
                    .get_many::<String>(arg.id)
                    .map(|vals| vals.cloned().collect())
                    .unwrap_or_default(),
            ),
            ArgKind::Trailing => args.insert_multi(
                arg.id,
                matches
                    .get_many::<String>(arg.id)
                    .map(|vals| vals.cloned().collect())
                    .unwrap_or_default(),
            ),
        }
    }
    Ok(args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_spec() -> CommandSpec {
        CommandSpec::leaf(
            "greet",
            "greet someone",
            vec![
                ArgSpec::positional("who", "name"),
                ArgSpec::opt("upper", "upper", "shout"),
            ],
            |_ctx, args| Ok(json!({ "hello": args.str("who"), "upper": args.flag("upper") })),
        )
    }

    // collect_args needs real clap matches; exercised end-to-end via the bin.
    #[test]
    fn handler_receives_declared_args_shape() {
        let spec = sample_spec();
        assert!(spec.handler.is_some());
        assert_eq!(spec.args.len(), 2);
    }
}
