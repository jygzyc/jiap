//! Static spec types shared by the generated registry (`engines_gen.rs`),
//! the engine runtime, and the command dispatcher.
//!
//! These types are deliberately `&'static`-friendly (no `String`, no `Vec`):
//! they exist as `static` initializers produced at COMPILE TIME by `build.rs`
//! from `config.json`. The CLI's entire knowledge of tool domains and
//! engines — command surfaces, launch commands, parseable launch parameters,
//! documentation — lives in these statics.

/// How an argument is parsed and presented on the command line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgKind {
    /// Boolean switch (`--force`).
    Flag,
    /// Single value (`--port 1234`).
    Value,
    /// Repeatable value (`--script a --script b`).
    Multi,
    /// Positional (`<FILE>`); `id` is the value name.
    Positional,
    /// Everything after the first positional, verbatim (server args).
    Trailing,
}

/// Static argument descriptor (mirrors the JSON `args` entries).
#[derive(Debug, Clone, Copy)]
pub struct ArgSpecS {
    pub id: &'static str,
    pub long: &'static str,
    pub kind: ArgKind,
    pub required: bool,
    pub values: &'static [&'static str],
    pub help: &'static str,
}

/// How a mapped request-body value is typed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapType {
    Str,
    U64,
    Bool,
    StrList,
}

/// When a mapping emits its field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapWhen {
    /// Only when the source arg was provided.
    Set,
    /// Always (falling back to `default` when the arg is absent).
    Always,
}

/// Compile-time constant defaults for `always` mappings.
#[derive(Debug, Clone, Copy)]
pub enum MapDefault {
    None,
    Str(&'static str),
    U64(u64),
    Bool(bool),
    EmptyList,
}

/// One `arg -> dotted body path` request mapping.
#[derive(Debug, Clone, Copy)]
pub struct FieldMap {
    pub arg: &'static str,
    pub field: &'static str,
    pub ty: MapType,
    pub when: MapWhen,
    pub default: MapDefault,
    /// bool mappings: emit `!flag` instead of `flag` (`--no-regex`).
    pub invert: bool,
}

/// An endpoint-backed command: POST /api/decx/<endpoint> with the body
/// assembled from `request` mappings.
#[derive(Debug, Clone, Copy)]
pub struct Route {
    pub endpoint: &'static str,
    pub request: &'static [FieldMap],
}

/// Static command spec: a group (`subs` non-empty) or a leaf. A leaf is
/// either endpoint-backed (`route`) or a local command (`local`: handler id
/// looked up in `commands::local`; runs in-process, never hits a server).
/// `engines` (empty = every engine the tool lists) narrows which engines
/// implement an endpoint leaf.
#[derive(Debug)]
pub struct CmdSpec {
    pub name: &'static str,
    pub about: &'static str,
    pub args: &'static [ArgSpecS],
    pub subs: &'static [CmdSpec],
    pub engines: &'static [&'static str],
    pub route: Option<&'static Route>,
    /// Handler id for local leaves (`java.android.device.system-services`).
    pub local: Option<&'static str>,
}

impl CmdSpec {
    /// Dotted paths of every leaf below (and including) this spec.
    pub fn leaf_paths(&self, prefix: &str, out: &mut Vec<String>) {
        let full = if prefix.is_empty() {
            self.name.to_string()
        } else {
            format!("{prefix}.{}", self.name)
        };
        if self.subs.is_empty() {
            out.push(full);
        } else {
            for sub in self.subs {
                sub.leaf_paths(&full, out);
            }
        }
    }

    /// Find a leaf by dotted path (`java.classes`), relative to this spec.
    pub fn find_leaf(&'static self, path: &str) -> Option<&'static CmdSpec> {
        let mut segments = path.split('.');
        let first = segments.next()?;
        if first != self.name {
            return None;
        }
        let rest = segments.collect::<Vec<_>>().join(".");
        if rest.is_empty() {
            return if self.subs.is_empty() { Some(self) } else { None };
        }
        self.subs
            .iter()
            .find_map(|sub| sub.find_leaf(&rest))
    }

    /// Whether an engine can serve this leaf: the leaf's narrowing list (if
    /// any) must contain the engine. Groups return false.
    pub fn supports_engine(&self, engine: &str) -> bool {
        self.engines.is_empty() || self.engines.contains(&engine)
    }
}

/// A tool domain registered at compile time from `config.json`: a top-level
/// command namespace (`java`, `binary`, ...) whose leaves POST endpoints on
/// the session's engine server.
#[derive(Debug)]
pub struct ToolSpec {
    pub name: &'static str,
    pub about: &'static str,
    /// Engines that can serve this tool's commands.
    pub engines: &'static [&'static str],
    pub commands: &'static [CmdSpec],
}

impl ToolSpec {
    /// Find a leaf by its path relative to the tool root: `classes` or
    /// nested `group.sub`.
    pub fn find_leaf(&'static self, path: &str) -> Option<&'static CmdSpec> {
        self.commands.iter().find_map(|c| c.find_leaf(path))
    }

    /// Dotted command paths this tool exposes (prefixed with the tool name).
    pub fn command_paths(&self) -> Vec<String> {
        let mut out = Vec::new();
        for cmd in self.commands {
            cmd.leaf_paths(self.name, &mut out);
        }
        out
    }

    /// Dotted command paths an engine can serve within this tool. Local
    /// leaves (in-process handlers) are excluded: engines only serve
    /// endpoint leaves.
    pub fn command_paths_for(&self, engine: &str) -> Vec<String> {
        let mut out = Vec::new();
        fn walk(cmd: &'static CmdSpec, prefix: &str, engine: &str, out: &mut Vec<String>) {
            let full = if prefix.is_empty() {
                cmd.name.to_string()
            } else {
                format!("{prefix}.{}", cmd.name)
            };
            if cmd.subs.is_empty() {
                if cmd.route.is_some() && cmd.supports_engine(engine) {
                    out.push(full);
                }
            } else {
                for sub in cmd.subs {
                    walk(sub, &full, engine, out);
                }
            }
        }
        for cmd in self.commands {
            walk(cmd, self.name, engine, &mut out);
        }
        out
    }
}

/// The compiled-in registry of tool domains (`decx java`, `decx binary`).
pub fn find_tool(name: &str) -> Option<&'static ToolSpec> {
    crate::engines_gen::TOOLS.iter().find(|t| t.name == name)
}

/// Resolve a dotted command path (`java.classes`) against the registry.
pub fn find_leaf(path: &str) -> Option<(&'static ToolSpec, &'static CmdSpec)> {
    let (tool_name, rest) = path.split_once('.')?;
    let tool = find_tool(tool_name)?;
    tool.find_leaf(rest).map(|leaf| (tool, leaf))
}

/// A parameter the launched engine server can parse (`--warm`, `--port`).
#[derive(Debug, Clone, Copy)]
pub struct ParamSpec {
    pub id: &'static str,
    pub long: &'static str,
    pub kind: ParamKind,
    pub help: &'static str,
    pub default: MapDefault,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamKind {
    Flag,
    Value,
}

/// How the engine binary is discovered and launched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryKind {
    /// A native executable (`decx-native-server[.exe]`).
    Program,
    /// A jar run under `java -jar` (`decx-server.jar`).
    JavaJar,
}

#[derive(Debug, Clone, Copy)]
pub struct BinarySpec {
    pub kind: BinaryKind,
    pub path: &'static str,
    pub env: &'static str,
    pub exe_suffix: bool,
    pub search_sibling: bool,
    pub search_dirs: &'static [&'static str],
}

#[derive(Debug, Clone, Copy)]
pub struct LaunchSpec {
    /// Command template with `{binary}` / `{target}` / `{port}` /
    /// `{java_heap}` placeholders.
    pub command: &'static [&'static str],
    /// Script files are appended as positional inputs.
    pub scripts: bool,
    /// The server accepts extra trailing args after the target.
    pub trailing_args: bool,
    pub params: &'static [ParamSpec],
}

/// One engine registered at compile time from `config.json`: a pure server
/// binary — discovery + launch knowledge + docs. Commands live on tools.
#[derive(Debug, Clone, Copy)]
pub struct EngineSpec {
    pub id: &'static str,
    pub description: &'static str,
    /// Longer documentation (queried via `decx engine show`).
    pub docs: &'static str,
    pub binary: BinarySpec,
    pub launch: LaunchSpec,
    /// Tool domains this engine serves (derived from `tools[].engines`).
    pub tools: &'static [&'static str],
}

impl EngineSpec {
    pub fn find_param(&self, id: &str) -> Option<&'static ParamSpec> {
        self.launch.params.iter().find(|p| p.id == id)
    }
}
