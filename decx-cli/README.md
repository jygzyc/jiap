# DECX CLI (Rust)

`decx` — Decompiler + X CLI, rewritten in Rust. This is the `decx-cli-dev`
branch rewrite of the former TypeScript CLI, redesigned around three
architectural pieces inspired by [opencli](https://github.com/jackwener/opencli)'s
adapter/registry model:

1. **Independent project manager** — every analysis target is a *project*
   with a supervised background server process and a monitored execution
   state (`starting / healthy / unreachable / stopped`), transition events,
   and a live `watch` stream.
2. **Unified tool integration surface** — every command group is a `Tool`
   registered in a `ToolRegistry` that assembles the CLI tree; new tools plug
   in by implementing one trait, and external CLI tools register through
   `decx tools register` and become reachable as `decx <name> [args...]`
   (opencli's `external register` equivalent).
3. **Pluggable analysis engines (adapter model)** — the opencli pattern
   applied to decompiler backends: every engine is one self-contained
   adapter file under `engine/adapters/` plus one line in the
   [`builtin()`](crates/decx-cli-core/src/engine/adapters/mod.rs) manifest.
   Two kinds behind one `Engine` trait: *server* engines (`jvm`
   decx-server.jar, `native` decx-native-server — long-lived DECX-contract
   HTTP servers) and *command* engines — one-shot CLI decompilers, with
   [kuna](https://github.com/Noelo-Lab/kuna) built in as the documented
   template. Select with `--engine` or `DECX_ENGINE`.

## Build and test

```bash
cargo build --release        # produces target/release/decx[.exe]
cargo test
```

Rust 1.80+ (edition 2021). The workspace has no C dependencies and no TLS
stack: server communication is a hand-rolled HTTP/1.1 client over
`std::net::TcpStream` (the DECX server is always on 127.0.0.1), and internet
downloads (self install, URL targets) delegate to the system `curl`.

## Command surface

```
decx project open <file> [--engine jvm|native] [--port N] [-n name]
                         [--script s.jadx.kts]... [--force] [--timeout secs]
                         [jadx passthrough args...]
decx project close [name] [--port N] [--all]
decx project list [--probe]          # projects + monitored state
decx project status [name] [--port N]
decx project check [--port N]        # binaries, port, server health
decx project watch [name] [--interval s]   # stream state transitions
decx project events [name] [--limit n]     # replay recorded transitions

decx code classes|search-global|class-context|class-source|method-source|
      method-context|method-cfg|search-class|search-method|xref-method|
      xref-class|xref-field|implementations|subclasses ...

decx android app manifest|launcher-activity|application|exported-components|
      deep-links|dynamic-receivers|framework-service-implementation|resources|
      resource-file|strings|aidl-interfaces ...
decx android device system-services [--serial S] [--adb-path P] [--grep K]
decx android device permission-info <permission> [--serial S]
decx android framework open [jar]    # collect/process/run: see parity notes

decx tools register <name> -- <command...>
decx tools list | remove <name> | run <name> [args...]
decx <name> [args...]                # registered external tool passthrough

decx engine list | show <id>         # inspect engine adapters (kind, capabilities, discovery)

decx self install [--prerelease] | update | status
```

`decx process ...` remains available as a hidden alias for `decx project ...`.

### Output contract (stable for AI agents)

- Successful results print **JSON on stdout** (`--format table` for humans).
- Progress notices and errors go to **stderr**; stdout stays parseable.
- Exit codes follow `sysexits.h`: `0` ok, `64` usage, `66` missing
  input/project, `69` server unavailable, `70` internal, `75` timeout.
- `DECX_DEBUG=1` logs HTTP traffic and debug details to stderr.

## Architecture

Workspace layout:

```
decx-cli/
├── crates/decx-cli-core/        # the architecture (library)
│   └── src/
│       ├── project/             # ← independent project manager
│       │   ├── model.rs         #   Project, ProjectState state machine
│       │   ├── store.rs         #   <DECX_HOME>/projects/*.json + events.jsonl
│       │   ├── manager.rs       #   ProjectManager: records, probes, events
│       │   └── monitor.rs       #   background monitor threads
│       ├── tools/               # ← unified tool integration surface
│       │   ├── mod.rs           #   Tool trait, ToolRegistry, ToolContext, AnalysisClient
│       │   ├── external.rs      #   external CLI registry (opencli-style)
│       │   ├── project_tool.rs  #   project/process command group
│       │   ├── code_tool.rs     #   code command group
│       │   ├── android_tool.rs  #   android app/device/framework
│       │   ├── engine_tool.rs   #   engine list/show (adapter introspection)
│       │   ├── tools_tool.rs    #   tools register/list/remove/run
│       │   ├── self_tool.rs     #   self install/update/status
│       │   └── adb.rs           #   adb client + output parsers
│       ├── engine/              # ← unified decompiler entry (adapter model)
│       │   ├── mod.rs           #   Engine protocol (query/capabilities) + EngineRegistry
│       │   ├── adapters/        #   one file per engine + the builtin() manifest
│       │   │   ├── jvm.rs       #     decx-server.jar server engine
│       │   │   ├── native.rs    #     decx-native-server server engine
│       │   │   └── kuna.rs      #     command-engine template (copy me)
│       │   └── launcher.rs      #   open flow, reuse decisions, health/exit wait
│       ├── client.rs            # DecxClient (all 26 endpoints)
│       ├── net.rs               # std-only HTTP/1.1 client + curl downloads
│       └── spawn.rs             # detached spawn, pid liveness, tree kill
└── crates/decx-cli/             # the `decx` binary (thin entrypoint)
```

### Project manager

`ProjectManager` (in `decx-cli-core::project`) owns every analysis project:

- **Records** persist under `DECX_HOME/projects/<name>.json` — target file,
  sha256, engine, pid, port, scripts, log path, and the last *observed*
  state, so any short-lived CLI invocation can report monitored state.
- **Probing** classifies each project through a pure state machine
  (`model::evaluate_state`): process dead → `stopped`; `/health` says
  `running` → `healthy`; alive but health failing → `starting` (never yet
  healthy) or `unreachable` (flapped after being healthy).
- **Monitors** are background threads (one per supervised project) that poll
  PID liveness + `/health`, persist observed state, append transitions to
  `<name>.events.jsonl`, and broadcast to subscribers. `project watch`
  subscribes to the live stream; `project events` replays the log.
- **Session reuse** mirrors the TypeScript semantics: an alive project with
  the same file hash + script set is reused; different scripts refuse until
  `--force`; `--force` kills same-name/same-hash servers with verified death
  before respawning (a failed kill aborts the spawn instead of orphaning).

### Tool adapter interface

New integrations implement one trait (`decx-cli-core::tools::Tool`) and
register in the `ToolRegistry` — the entrypoint assembles the clap tree from
the registry, routes dispatch, applies the output contract, and maps errors
to exit codes:

```rust
pub trait Tool: Send + Sync {
    fn id(&self) -> &'static str;
    fn commands(&self) -> Vec<clap::Command>;   // subcommand tree (aliases ok)
    fn run(&self, ctx: &ToolContext, m: &clap::ArgMatches)
        -> Result<serde_json::Value, DecxError>;
}
```

`ToolContext` hands the tool the project manager, engine registry, home
paths, and output format. Analysis tools resolve their server with the
shared `-s/--session` / `--port` / auto-select resolution.

External CLI tools need no code at all: `decx tools register <name> -- <cmd>`
stores the registration in `DECX_HOME/tools.json` and the entrypoint spawns
it as `decx <name> [args...]` with inherited stdio and propagated exit code —
the same unified-surface idea as opencli's `external register`.

### Engine adapters (unified decompiler entry)

Engines follow the opencli adapter model — the `cli({ ... func })` declaration
of engines. One self-contained file per engine under
`crates/decx-cli-core/src/engine/adapters/`, one line in the `builtin()`
manifest; the runtime owns project supervision, background monitoring,
argument parsing, the DECX envelope, and exit codes. Adapters are pure:
they locate their binary, build launch commands, and answer queries.

The protocol (`engine/mod.rs`):

```rust
pub trait Engine: Send + Sync {
    fn id(&self) -> &'static str;              // "jvm" | "native" | "kuna" | ...
    fn description(&self) -> &'static str;
    fn kind(&self) -> EngineKind;              // Server (HTTP) | Command (one-shot)
    fn capabilities(&self) -> &'static [&'static str];  // endpoints a command engine answers

    fn resolve_binary(&self, home: &Path) -> DecxResult<PathBuf>;   // discovery
    fn validate(&self, spec: &TargetSpec) -> DecxResult<()>;        // unsupported options
    fn build_command(&self, binary: &Path, spec: &TargetSpec) -> DecxResult<Command>;
    // launch plan: server spawn line, or the analyze job run as a
    // monitored background process whose exit code drives the project state

    fn query(&self, project: &Project, endpoint: &str, key: Option<&str>) -> DecxResult<Value>;
    // the func(args) of engines: DECX endpoint → invocation; stdout is
    // wrapped in the DECX envelope by execute_query()
}
```

Two kinds:

- **Server engines** run a long-lived HTTP server speaking the DECX contract.
  `jvm` mirrors the TypeScript launcher exactly (heap = 2/3 of machine
  memory, jadx passthrough normalization: strip `--deobf`, inject
  `--show-bad-code --no-imports -Pdex-input.verify-checksum=no`, default
  `--rename-flags case,valid`, strip the `printable` token, positional
  `.jadx.kts` scripts). `native` rejects `--script` and ignores jadx flags.
  Servers spawn detached (Unix: own process group; Windows:
  `DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP`) with stdout/stderr appended
  to `DECX_HOME/logs/<name>.log`, and are killed with verified process-tree
  death. Queries flow through the HTTP client.
- **Command engines** are one-shot CLI decompilers. `project open` runs the
  adapter's analyze command as a background job with logs and a stderr
  heartbeat; the exit code drives the project state machine (`starting` →
  `healthy`/failed), and on timeout the record is kept with the pid tracked.
  `decx code` / `decx android app` route every endpoint through the
  `AnalysisClient`: HTTP for server projects, the adapter's `query` handler
  otherwise; unimplemented endpoints fail with `UNSUPPORTED_BY_ENGINE`
  listing the adapter's capabilities.

**Adding an engine = copy the template + one manifest line.**
`adapters/kuna.rs` is the documented reference (identity, binary discovery,
analyze command, two query handlers — ~100 lines with comments). Copy it,
adjust four things, add one line to `adapters::builtin()`:

```rust
// engine/adapters/mydec.rs        ← copy of kuna.rs, adjusted
pub struct MyDec;
impl Engine for MyDec { /* id/kind/capabilities, find_binary, build_command, query */ }

// engine/adapters/mod.rs
pub fn builtin() -> Vec<Arc<dyn Engine>> {
    vec![
        Arc::new(jvm::JvmEngine),
        Arc::new(native::NativeEngine),
        Arc::new(kuna::Kuna),
        Arc::new(mydec::MyDec),    // ← the one line
    ]
}
```

Project supervision, monitoring, `project check`, and the `decx code`
routing pick it up automatically.

Usage with the built-in kuna adapter:

```text
decx project open ./a.out --engine kuna
decx code method-source main
decx engine show kuna        # kind, capabilities, binary discovery
```

Binary discovery:

| Engine | Lookup order |
|---|---|
| `jvm` | `DECX_SERVER_HOME` (file or dir) → `<DECX_HOME>/bin/decx-server.jar` |
| `native` | `DECX_NATIVE_SERVER` (file or dir) → `<DECX_HOME>/bin/decx-native-server[.exe]` → dev checkout `decx-native/target/release/` near the working directory |
| `kuna` | `DECX_KUNA` (file or dir) → `kuna` on PATH |
| your adapter | whatever `resolve_binary` implements (`engine::resolve_program` gives shell-like PATH lookup with `.exe` fallback) |

## Parity with the TypeScript CLI

Ported: session/project lifecycle (open/close/list/status/check + new
watch/events), all `code` analysis commands, all `android app` commands,
`android device` (system-services, permission-info), `android framework open`,
`self install/update/status`, URL target downloads, jar version probing and
skip-if-current install.

Not ported yet (fail with a clear `NOT_PORTED` error): the framework
`collect`/`process`/`run` device-pull and image-extraction pipeline, and the
`self skills` installer. The npm update-notifier is intentionally dropped
(Rust builds distribute via cargo/release binaries).
