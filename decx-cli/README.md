# DECX CLI (Rust)

`decx` â Decompiler + X CLI, rewritten in Rust. This is the `decx-cli-dev`
branch rewrite of the former TypeScript CLI, redesigned around three
architectural pieces inspired by [opencli](https://github.com/jackwener/opencli)'s
adapter/registry model:

1. **Tool layer (`tools/`)** â capabilities and interfaces. Every command
   group is an internal, self-implemented `Tool` registered in a
   `ToolRegistry` that assembles the CLI tree; external CLIs join the same
   surface through `decx tools register` (opencli's `external register`).
2. **Session layer (`session/`)** â manages the sessions produced by tool
   invocations: `SessionManager` records every engine run
   (`DECX_HOME/sessions/`), supervises it through a health/exit state machine
   (`starting / healthy / unreachable / stopped`), streams transition
   events (`session watch` / `session events`), and owns the open/close/reuse
   policy (`session::lifecycle`).
3. **Engine layer (`engine/`)** â governs the plugged-in tool server
   backends: one self-contained adapter file per engine under
   `engine/adapters/` plus one line in the
   [`builtin()`](crates/decx-cli-core/src/engine/adapters/mod.rs) manifest.
   Two kinds behind one `Engine` trait: *server* engines (`jvm`
   decx-server.jar, `native` decx-native-server â long-lived DECX-contract
   HTTP servers) and *command* engines â one-shot CLI decompilers, with
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
decx session open <file> [--engine jvm|native] [--port N] [-n name]
                         [--script s.jadx.kts]... [--force] [--timeout secs]
                         [jadx passthrough args...]
decx session close [name] [--port N] [--all]
decx session list [--probe]          # sessions + monitored state
decx session status [name] [--port N]
decx session check [--port N]        # binaries, port, server health
decx session watch [name] [--interval s]   # stream state transitions
decx session events [name] [--limit n]     # replay recorded transitions

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

`decx project ...` and `decx process ...` remain available as hidden aliases
for `decx session ...` (the pre-rewrite command names).

### Output contract (stable for AI agents)

- Successful results print **JSON on stdout** (`--format table` for humans).
- Progress notices and errors go to **stderr**; stdout stays parseable.
- Exit codes follow `sysexits.h`: `0` ok, `64` usage, `66` missing
  input/session, `69` server unavailable, `70` internal, `75` timeout.
- `DECX_DEBUG=1` logs HTTP traffic and debug details to stderr.

## Architecture

Workspace layout:

```
decx-cli/
│   ├── crates/decx-cli-core/        # the architecture (library)
│   │   └── src/
│   │       ├── tools/               # ← tool layer: capabilities & interfaces
│   │       │   ├── mod.rs           #   Tool trait, ToolRegistry, ToolContext, AnalysisClient
│   │       │   ├── external.rs      #   external CLI registry (opencli-style)
│   │       │   ├── session_tool.rs  #   session (alias project/process) commands
│   │       │   ├── code_tool.rs     #   code command group
│   │       │   ├── android_tool.rs  #   android app/device/framework
│   │       │   ├── engine_tool.rs   #   engine list/show (adapter introspection)
│   │       │   ├── tools_tool.rs    #   tools register/list/remove/run
│   │       │   ├── self_tool.rs     #   self install/update/status
│   │       │   └── adb.rs           #   adb client + output parsers
│   │       ├── session/             # ← session layer: sessions from tool invocations
│   │       │   ├── lifecycle.rs     #   open flow: target resolution, reuse policy
│   │       │   ├── model.rs         #   Session, SessionState state machine
│   │       │   ├── store.rs         #   <DECX_HOME>/sessions/*.json + events.jsonl
│   │       │   ├── manager.rs       #   SessionManager: records, probes, events
│   │       │   └── monitor.rs       #   background monitor threads
│   │       ├── engine/              # ← engine layer: governs plugged-in tool server backends
│   │       │   ├── mod.rs           #   Engine protocol + EngineRegistry::status
│   │       │   ├── launcher.rs      #   launch/health runtime (server readiness probes)
│   │       │   └── adapters/        #   one file per engine + the builtin() manifest
│   │       │       ├── jvm.rs       #     decx-server.jar server engine
│   │       │       ├── native.rs    #     decx-native-server server engine
│   │       │       └── kuna.rs      #     command-engine template (copy me)
│   │       ├── client.rs            # DecxClient (all 26 endpoints)
│   │       ├── net.rs               # std-only HTTP/1.1 client + curl downloads
│   │       └── spawn.rs             # detached spawn, pid liveness, tree kill
└── crates/decx-cli/             # the `decx` binary (thin entrypoint)
```

### Session layer

`SessionManager` (in `decx-cli-core::session`) manages the sessions produced
by tool invocations â every engine run opened through any tool
(`decx session open`, `decx android framework open`, ...) is one session:

- **Records** persist under `DECX_HOME/sessions/<name>.json` â target file,
  sha256, engine + kind, pid, port, scripts, log path, the invoking tool
  (`origin`), and the last *observed* state, so any short-lived CLI
  invocation can report monitored state.
- **Lifecycle** (`session::lifecycle`) owns the open flow: target resolution
  (URL inputs download), the reuse policy (same file hash + script set
  reuses, different scripts refuse until `--force`, `--force` replaces
  same-name/same-hash sessions with verified process-tree death), and the
  orchestration that delegates launches to the engine layer.
- **Probing** classifies each session through a pure state machine
  (`model::evaluate_state`): process dead â `stopped`; `/health` says
  `running` â `healthy`; alive but health failing â `starting` (never yet
  healthy) or `unreachable` (flapped after being healthy). Command-engine
  sessions record the analyze exit instead.
- **Monitors** are background threads (one per supervised session) that
  probe on an interval, persist observed state, append transitions to
  `<name>.events.jsonl`, and broadcast to subscribers. `session watch`
  subscribes to the live stream; `session events` replays the log.

### Tool adapter interface

New integrations implement one trait (`decx-cli-core::tools::Tool`) and
register in the `ToolRegistry` â the entrypoint assembles the clap tree from
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

`ToolContext` hands the tool the session manager, engine registry, home
paths, and output format. Analysis tools resolve their server with the
shared `-s/--session` / `--port` / auto-select resolution.

External CLI tools need no code at all: `decx tools register <name> -- <cmd>`
stores the registration in `DECX_HOME/tools.json` and the entrypoint spawns
it as `decx <name> [args...]` with inherited stdio and propagated exit code â
the same unified-surface idea as opencli's `external register`.

### Engine adapters (unified decompiler entry)

Engines follow the opencli adapter model â the `cli({ ... func })` declaration
of engines. One self-contained file per engine under
`crates/decx-cli-core/src/engine/adapters/`, one line in the `builtin()`
manifest; the runtime owns session supervision, background monitoring,
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
    // monitored background process whose exit code drives the session state

    fn query(&self, session: &Session, endpoint: &str, key: Option<&str>) -> DecxResult<Value>;
    // the func(args) of engines: DECX endpoint â invocation; stdout is
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
- **Command engines** are one-shot CLI decompilers. `session open` runs the
  adapter's analyze command as a background job with logs and a stderr
  heartbeat; the exit code drives the session state machine (`starting` â
  `healthy`/failed), and on timeout the record is kept with the pid tracked.
  `decx code` / `decx android app` route every endpoint through the
  `AnalysisClient`: HTTP for server sessions, the adapter's `query` handler
  otherwise; unimplemented endpoints fail with `UNSUPPORTED_BY_ENGINE`
  listing the adapter's capabilities.

**Adding an engine = copy the template + one manifest line.**
`adapters/kuna.rs` is the documented reference (identity, binary discovery,
analyze command, two query handlers â ~100 lines with comments). Copy it,
adjust four things, add one line to `adapters::builtin()`:

```rust
// engine/adapters/mydec.rs        â copy of kuna.rs, adjusted
pub struct MyDec;
impl Engine for MyDec { /* id/kind/capabilities, find_binary, build_command, query */ }

// engine/adapters/mod.rs
pub fn builtin() -> Vec<Arc<dyn Engine>> {
    vec![
        Arc::new(jvm::JvmEngine),
        Arc::new(native::NativeEngine),
        Arc::new(kuna::Kuna),
        Arc::new(mydec::MyDec),    // â the one line
    ]
}
```

Session supervision, monitoring, `session check`, and the `decx code`
routing pick it up automatically.

Usage with the built-in kuna adapter:

```text
decx session open ./a.out --engine kuna
decx code method-source main
decx engine show kuna        # kind, capabilities, binary discovery
```

Binary discovery:

| Engine | Lookup order |
|---|---|
| `jvm` | `DECX_SERVER_HOME` (file or dir) â `<DECX_HOME>/bin/decx-server.jar` |
| `native` | `DECX_NATIVE_SERVER` (file or dir) â `<DECX_HOME>/bin/decx-native-server[.exe]` â dev checkout `decx-native/target/release/` near the working directory |
| `kuna` | `DECX_KUNA` (file or dir) â `kuna` on PATH |
| your adapter | whatever `resolve_binary` implements (`engine::resolve_program` gives shell-like PATH lookup with `.exe` fallback) |

## Parity with the TypeScript CLI

Ported: session lifecycle (open/close/list/status/check + new
watch/events), all `code` analysis commands, all `android app` commands,
`android device` (system-services, permission-info), `android framework open`,
`self install/update/status`, URL target downloads, jar version probing and
skip-if-current install.

Not ported yet (fail with a clear `NOT_PORTED` error): the framework
`collect`/`process`/`run` device-pull and image-extraction pipeline, and the
`self skills` installer. The npm update-notifier is intentionally dropped
(Rust builds distribute via cargo/release binaries).
