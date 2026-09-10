//! The interface protocol between internal command groups / the compiled-in
//! tool domains and the unified command line.
//!
//! A command group never writes CLI plumbing. It declares an [`Interface`]
//! — pure data: command names, argument descriptors, help text — plus one
//! [`Handler`] per leaf command. Tool-domain leaves (from
//! `engines_gen::TOOLS`, compiled in from `config.json`) carry a static
//! leaf reference instead of a handler: the generic dispatcher assembles
//! the HTTP request body from the compile-time field mappings and posts it
//! to the session's engine server (`POST /api/decx/<endpoint>`). The CLI
//! owns the single generic engine that compiles any set of interfaces into
//! a clap tree and dispatches parsed arguments back to leaves.
//!
//! ```text
//! internal (Rust)                    tool domains (config.json)     cli side
//! ─────────────────                  ───────────────────────────    ─────────
//! Interface { commands, args }       statics (engines_gen.rs)  ───► build_cli() → clap tree
//! Handler(&ToolCtx, &Args)           static_leaf(tool, cmd)    ◄─── dispatch → handler | HTTP

use serde_json::Value;

use crate::commands::{Args, ToolContext};
use crate::error::{DecxError, DecxResult};
use crate::spec::{ArgKind as ArgKindS, ArgSpecS, CmdSpec, ToolSpec};

/// How an argument is parsed and presented (owned mirror of
/// [`crate::spec::ArgKind`] for runtime construction).
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
#[derive(Debug, Clone)]
pub struct ArgSpec {
    pub id: &'static str,
    /// `--<long>`; ignored for [`ArgKind::Positional`] / `Trailing`.
    pub long: &'static str,
    pub kind: ArgKind,
    pub required: bool,
    /// Allowed values (enum validation); empty = free-form.
    pub values: Vec<&'static str>,
    pub help: &'static str,
}

impl ArgSpec {
    pub fn positional(id: &'static str, help: &'static str) -> Self {
        Self {
            id,
            long: "",
            kind: ArgKind::Positional,
            required: true,
            values: Vec::new(),
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
            values: Vec::new(),
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
            values: Vec::new(),
            help,
        }
    }

    pub fn multi(id: &'static str, long: &'static str, help: &'static str) -> Self {
        Self {
            id,
            long,
            kind: ArgKind::Multi,
            required: false,
            values: Vec::new(),
            help,
        }
    }

    pub fn trailing(id: &'static str, help: &'static str) -> Self {
        Self {
            id,
            long: "",
            kind: ArgKind::Trailing,
            required: false,
            values: Vec::new(),
            help,
        }
    }

    /// Restrict to an enum of values (still optional unless
    /// [`ArgSpec::required_opt`] is used instead).
    pub fn one_of(id: &'static str, long: &'static str, values: &[&'static str], help: &'static str) -> Self {
        Self {
            values: values.to_vec(),
            ..Self::opt(id, long, help)
        }
    }
}

/// One command (leaf: has a handler or a static tool leaf; group: has
/// subcommands).
#[derive(Clone)]
pub struct CommandSpec {
    pub name: &'static str,
    pub about: &'static str,
    pub args: Vec<ArgSpec>,
    pub subcommands: Vec<CommandSpec>,
    pub handler: Option<Handler>,
    /// Tool-domain leaf (from `engines_gen::TOOLS`): the static tool + leaf
    /// specs the dispatcher uses for engine gating and the request body.
    pub static_leaf: Option<(&'static ToolSpec, &'static CmdSpec)>,
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
            static_leaf: None,
        }
    }

    /// Leaf whose implementation lives in an engine server: the static
    /// tool/leaf specs carry the endpoint, the request mappings, and the
    /// engine narrowing for dispatch.
    pub fn leaf_static(
        tool: &'static ToolSpec,
        cmd: &'static CmdSpec,
        args: Vec<ArgSpec>,
    ) -> Self {
        Self {
            name: cmd.name,
            about: cmd.about,
            args,
            subcommands: Vec::new(),
            handler: None,
            static_leaf: Some((tool, cmd)),
        }
    }

    pub fn group(name: &'static str, about: &'static str, subcommands: Vec<CommandSpec>) -> Self {
        Self {
            name,
            about,
            args: Vec::new(),
            subcommands,
            handler: None,
            static_leaf: None,
        }
    }
}

/// The interface a tool exposes to the CLI.
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
            let own: Vec<ArgSpec> = inherited.iter().chain(spec.args.iter()).cloned().collect();
            if spec.subcommands.is_empty() {
                CommandSpec {
                    args: own,
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

/// Walk a declared command tree alongside parsed matches and run the leaf
/// (native handler or engine route). Generic — this is the whole dispatch
/// logic of the CLI. The dotted leaf path (`code.classes`) is threaded
/// through so engine-backed leaves can look up the SESSION ENGINE's own
/// declaration of that command.
pub fn run_command(
    spec: &CommandSpec,
    common: &[ArgSpec],
    matches: &clap::ArgMatches,
    ctx: &ToolContext,
) -> DecxResult<Value> {
    run_command_at(spec, common, matches, ctx, &mut String::new())
}

fn run_command_at(
    spec: &CommandSpec,
    common: &[ArgSpec],
    matches: &clap::ArgMatches,
    ctx: &ToolContext,
    path: &mut String,
) -> DecxResult<Value> {
    if !path.is_empty() {
        path.push('.');
    }
    path.push_str(spec.name);
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
        return run_command_at(sub_spec, common, sub, ctx, path);
    }
    let args = collect_args(spec, common, matches)?;
    if let Some((tool, leaf)) = &spec.static_leaf {
        return crate::commands::run_route(ctx, tool, leaf, &args);
    }
    let handler = spec.handler.ok_or_else(|| {
        DecxError::usage(format!("'{spec_name}' has no handler", spec_name = spec.name))
    })?;
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
            ArgKind::Multi | ArgKind::Trailing => args.insert_multi(
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

// ── statics → runtime conversion (engine contract) ─────────────────────────

// -- statics -> runtime conversion (tool domains) ------------------------------

/// Convert one compile-time [`ToolSpec`] (from `engines_gen::TOOLS`) into
/// the runtime interface model: common args are already baked into the
/// static leaf args by build.rs, so this is a straight tree conversion.
pub fn tools_interfaces(tools: &'static [ToolSpec]) -> Vec<Interface> {
    tools
        .iter()
        .map(|tool| Interface {
            tool: tool.name,
            description: tool.about,
            common_args: Vec::new(),
            commands: tool.commands.iter().map(|c| from_static(tool, c)).collect(),
        })
        .collect()
}

fn from_static(tool: &'static ToolSpec, cmd: &'static CmdSpec) -> CommandSpec {
    if cmd.subs.is_empty() {
        let args = cmd.args.iter().map(from_static_arg).collect();
        if cmd.route.is_some() {
            CommandSpec::leaf_static(tool, cmd, args)
        } else if let Some(id) = cmd.local {
            // Local tool-domain command: implemented in-process by a Rust
            // handler (adb device inspection, ...). The id is bound in the
            // `commands::local` registry — a config referencing an unknown
            // id is a build/programming error, caught at startup.
            let handler = crate::commands::local::lookup(id).unwrap_or_else(|| {
                panic!(
                    "config.json local command '{}' references an unregistered handler",
                    id
                )
            });
            CommandSpec::leaf(cmd.name, cmd.about, args, handler)
        } else {
            CommandSpec::group(cmd.name, cmd.about, Vec::new())
        }
    } else {
        CommandSpec::group(
            cmd.name,
            cmd.about,
            cmd.subs.iter().map(|sub| from_static(tool, sub)).collect(),
        )
    }
}

fn from_static_arg(arg: &'static ArgSpecS) -> ArgSpec {
    ArgSpec {
        id: arg.id,
        long: arg.long,
        kind: match arg.kind {
            ArgKindS::Flag => ArgKind::Flag,
            ArgKindS::Value => ArgKind::Value,
            ArgKindS::Multi => ArgKind::Multi,
            ArgKindS::Positional => ArgKind::Positional,
            ArgKindS::Trailing => ArgKind::Trailing,
        },
        required: arg.required,
        values: arg.values.to_vec(),
        help: arg.help,
    }
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
            |_ctx, args| Ok(json!({ "hello": args.str("who")?, "upper": args.flag("upper") })),
        )
    }

    #[test]
    fn handler_receives_declared_args_shape() {
        let spec = sample_spec();
        assert!(spec.handler.is_some());
        assert!(spec.static_leaf.is_none());
        assert_eq!(spec.args.len(), 2);
    }

    #[test]
    fn tools_interfaces_carry_static_leaves_and_common_args() {
        let ifaces = tools_interfaces(crate::engines_gen::TOOLS);
        let java = ifaces.iter().find(|i| i.tool == "java").unwrap();
        let materialized = java.materialize();
        let root = materialized.commands.first().unwrap();
        let classes = root.subcommands.iter().find(|c| c.name == "classes").unwrap();
        assert!(classes.handler.is_none());
        let (tool, leaf) = classes.static_leaf.unwrap();
        assert_eq!(tool.name, "java");
        assert_eq!(leaf.route.unwrap().endpoint, "get_classes");
        // common args (session/port/page) were baked in by build.rs
        let ids: Vec<&str> = classes.args.iter().map(|a| a.id).collect();
        assert!(ids.contains(&"session"));
        assert!(ids.contains(&"page"));
    }

    #[test]
    fn every_tool_leaf_is_dispatchable() {
        // Every leaf must be either an engine-routed static leaf or a local
        // handler leaf — never a bare group conversion.
        fn walk(specs: &[CommandSpec], out: &mut Vec<(&'static str, bool, bool)>) {
            for c in specs {
                if c.subcommands.is_empty() {
                    out.push((c.name, c.static_leaf.is_some(), c.handler.is_some()));
                } else {
                    walk(&c.subcommands, out);
                }
            }
        }
        let ifaces = tools_interfaces(crate::engines_gen::TOOLS);
        let mut flags = Vec::new();
        for iface in &ifaces {
            let m = iface.materialize();
            walk(&m.commands, &mut flags);
        }
        assert!(!flags.is_empty());
        assert!(flags.iter().all(|(_, s, h)| *s ^ *h));

        // The android device commands are local handler leaves.
        let java = ifaces
            .iter()
            .find(|i| i.tool == "java")
            .unwrap()
            .materialize();
        // materialize() wraps the commands in one root spec named "java"
        let root = &java.commands[0];
        assert_eq!(root.name, "java");
        let android = root
            .subcommands
            .iter()
            .find(|c| c.name == "android")
            .unwrap();
        let device = android
            .subcommands
            .iter()
            .find(|c| c.name == "device")
            .unwrap();
        for leaf in &device.subcommands {
            assert!(leaf.handler.is_some(), "{} must be a local handler leaf", leaf.name);
            assert!(leaf.static_leaf.is_none());
            // local leaves carry NO baked common args
            let ids: Vec<&str> = leaf.args.iter().map(|a| a.id).collect();
            assert!(!ids.contains(&"session"));
            assert!(!ids.contains(&"page"));
        }
    }
}
