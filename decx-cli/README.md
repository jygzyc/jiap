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
3. **Pluggable analysis engines** — two kinds behind one `Engine` trait:
   *server* engines (built-in `jvm` decx-server.jar and `native`
   decx-native-server — long-lived HTTP servers speaking the DECX contract)
   and *command* engines: one-shot CLI decompilers such as
   [kuna](https://github.com/Noelo-Lab/kuna), registered declaratively with
   `decx engine register` (no recompilation) and queried through the same
   `decx code` surface via per-endpoint command templates. Select with
   `--engine` or `DECX_ENGINE`.

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

decx engine register <id> [--server] [--description D] -- <command...>
decx engine query <id> <endpoint> -- <command...>
decx engine list | show <id> | remove <id>

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
│       │   ├── engine_tool.rs   #   engine register/query/list/show/remove
│       │   ├── tools_tool.rs    #   tools register/list/remove/run
│       │   ├── self_tool.rs     #   self install/update/status
│       │   └── adb.rs           #   adb client + output parsers
│       ├── engine/              # ← pluggable analysis engines (unified decompiler entry)
│       │   ├── mod.rs           #   Engine trait (server/command kinds) + EngineRegistry
│       │   ├── foreign.rs       #   externally registered engines (engines.json + templates)
│       │   ├── jvm.rs           #   decx-server.jar spawn logic
│       │   ├── native.rs        #   decx-native-server spawn logic
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

### Engine adapters

`Engine` implementations own binary discovery and launch assembly for an
analysis backend. Two kinds exist:

- **Server engines** run a long-lived HTTP server speaking the DECX contract.
  `jvm` mirrors the TypeScript launcher exactly (heap = 2/3 of machine
  memory, jadx passthrough normalization: strip `--deobf`, inject
  `--show-bad-code --no-imports -Pdex-input.verify-checksum=no`, default
  `--rename-flags case,valid`, strip the `printable` token, positional
  `.jadx.kts` scripts). `native` rejects `--script` and ignores jadx flags.
  Servers spawn detached (Unix: own process group; Windows:
  `DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP`) with stdout/stderr appended
  to `DECX_HOME/logs/<name>.log`, and are killed with verified process-tree
  death.
- **Command engines** are one-shot CLI decompilers (kuna-style). `project
  open` runs the registered launch template as a background job with logs and
  a stderr heartbeat, records the exit code through the project state machine
  (`starting` → `healthy`/failed), and keeps the record on timeout.
  `decx code` / `decx android app` route every endpoint through the
  `AnalysisClient`: HTTP for server projects, the engine's registered
  command template otherwise; endpoints without a template fail with
  `UNSUPPORTED_BY_ENGINE` naming what is configured.

External engines register declaratively (`DECX_HOME/engines.json`) — no
recompilation. Templates are argv arrays spawned directly (no shell, no
quoting); exact-match tokens may be `{target}` (absolute target path),
`{port}` (server engines), and `{key}` (query templates: the function/class
key):

```text
decx engine register kuna -- kuna decompile-project {target}
decx engine query kuna get_method_source -- kuna decompile {target} {key}
decx engine query kuna get_class_source -- kuna decompile-project {target}
decx project open ./a.out --engine kuna
decx code method-source main
```

Binary discovery:

| Engine | Lookup order |
|---|---|
| `jvm` | `DECX_SERVER_HOME` (file or dir) → `<DECX_HOME>/bin/decx-server.jar` |
| `native` | `DECX_NATIVE_SERVER` (file or dir) → `<DECX_HOME>/bin/decx-native-server[.exe]` → dev checkout `decx-native/target/release/` near the working directory |
| foreign | first template token resolved like a shell: paths pass through, bare names via PATH (`dir/<name>[.exe]`) |

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
