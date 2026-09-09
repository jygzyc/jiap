# AGENTS.md

Coding agent instructions for the DECX repository.

## Repository Purpose

DECX (`Decompiler + X`) is an AI-oriented analysis layer built on top of JADX.
The repository contains:

- A Kotlin HTTP analysis server shared by plugin mode and standalone mode
- A JADX GUI plugin that starts the DECX server and an in-process Kotlin MCP server
- A standalone `decx-server` fat JAR for headless analysis
- A Rust CLI workspace that starts and talks to `decx-server`
- AI skill definitions under `skills/` for DECX-driven analysis workflows

Primary request flow:

```text
AI Assistant / CLI
  -> MCP or direct HTTP
  -> DECX HTTP server
  -> DecxApi
  -> JADX decompiler state
```

## Repository Layout

| Path | Stack | Role |
|---|---|---|
| `decx/decx-core/` | Kotlin, JVM 17 | Shared API, HTTP transport, services, models, utilities |
| `decx/decx-plugin/` | Kotlin, Shadow JAR | JADX GUI plugin, lifecycle, UI, in-process MCP server management |
| `decx/decx-server/` | Kotlin, Shadow JAR | Standalone headless server with `DecxServerApp` main class |
| `decx-cli/` | Rust (workspace, no external C deps) | User CLI: project manager with background monitoring, analysis commands, pluggable tool registry |
| `skills/decx-cli/` | Skill `decx-cli` | DECX CLI usage, general analysis, and workflow routing |
| `skills/decx-vulnhunt/` | Skill `decx-vulnhunt` | Android vulnerability hunting workflow (App + Framework tracks) |
| `skills/decx-report/` | Skill `decx-report` | Report generation from finalized DECX analysis graph findings |
| `skills/decx-poc/` | Skill `decx-poc` | PoC app construction workflow |

## What Is Actually Implemented

### Kotlin server capabilities

`decx-core` exposes these HTTP endpoints through `DecxRoutes` and `RouteHandler`:

- Common code analysis:
  `get_classes`, `get_class_context`, `get_class_source`, `search_global_key`, `search_class_key`,
  `search_method`, `get_method_source`, `get_method_context`, `get_method_cfg`, `get_method_xref`, `get_field_xref`,
  `get_class_xref`, `get_implementations`, `get_subclasses`
- Android app analysis:
  `get_aidl_interfaces`, `get_app_manifest`, `get_main_activity`, `get_application`,
  `get_exported_components`, `get_deep_links`, `get_dynamic_receivers`,
  `get_all_resources`, `get_resource_file`, `get_strings`
- Android framework analysis:
  `get_system_service_impl`
- Health endpoint:
  `GET /health`

### Plugin responsibilities

The JADX plugin does more than just expose the server:

- Waits until the decompiler is ready before creating DECX services
- Initializes preferences and server port
- Starts the embedded DECX HTTP server
- Starts and stops the in-process Kotlin MCP HTTP server on `serverPort + 1`
- Provides UI and restart hooks through `DecxUIManager`
- Bounds decompiler memory on headless servers: a byte-capped LRU code cache (`decx.decompile.cacheMaxBytes` → default `min(4G, -Xmx/2)`) plus a backpressured daemon that unloads evicted classes; see `DecompileGuard`

### CLI responsibilities

`decx-cli/` is a Rust workspace (crate `decx-cli-core` + binary `decx`)
redesigned around three pieces, following the opencli adapter/registry model:

- An **independent project manager** (`decx-cli-core::project`): every
  analysis target is a project record under `DECX_HOME/projects/<name>.json`
  (file, sha256, engine, pid, port, scripts, log path, observed state),
  supervised by background monitor threads that poll PID liveness + `/health`
  and drive a state machine (`starting | healthy | unreachable | stopped`).
  State transitions append to `<name>.events.jsonl` and surface through
  `decx project list --probe`, `decx project status`, `decx project events`,
  and the live `decx project watch` stream.
- A **unified tool integration surface** (`decx-cli-core::tools`): every
  command group implements the `Tool` trait (`id`, `commands`, `run`) and
  registers in the `ToolRegistry`, which assembles the clap tree and routes
  dispatch; external CLI tools register through `decx tools register <name>
  -- <command...>` (stored in `DECX_HOME/tools.json`) and become reachable as
  top-level `decx <name> [args...]` passthrough with inherited stdio and
  propagated exit codes.
- **Pluggable analysis engines** (`decx-cli-core::engine`), two kinds behind
  one `Engine` trait: *server* engines (built-in `jvm` decx-server.jar and
  `native` decx-native-server — long-lived DECX-contract HTTP servers) and
  *command* engines (one-shot CLI decompilers such as kuna). External
  engines register declaratively via `decx engine register` into
  `DECX_HOME/engines.json` (argv templates with `{target}`/`{port}`/`{key}`
  placeholders; `decx engine query <id> <endpoint> -- <cmd>` maps analysis
  endpoints to templates) — no recompilation. `project open` runs command
  engines as monitored background analyze jobs (exit code → state machine);
  `decx code`/`decx android app` route endpoints through `AnalysisClient`:
  HTTP for server projects, the registered template otherwise (missing
  templates fail with `UNSUPPORTED_BY_ENGINE` listing what is configured).
  Select with `--engine` or `DECX_ENGINE` (default `jvm`).

Current top-level commands: `project` (alias `process`), `code`, `android`,
`engine`, `tools`, `self`.

Notable details:

- `decx project open <file>` launches `java -jar decx-server.jar ...` (jvm)
  or `decx-native-server <target> --port N` (native); the JVM gets `-Xmx` =
  2/3 of machine memory rounded down
- `open` accepts `--script <s.jadx.kts>` (repeatable, JVM engine only —
  native rejects it); scripts run at decompile time via the bundled
  `jadx-script-kotlin` plugin
- Session reuse is keyed on the target file hash **plus** the exact script
  set; opening the same file with a different script set errors until
  `--force`
- `--force` replaces alive projects matching the same name **or** the same
  file hash: their process trees are killed with verified death before the
  new server starts. A failed kill aborts the spawn (record kept, pid
  reported) instead of leaking orphan processes; `project close` keeps the
  record on failed kills too
- While waiting for the server to become healthy, `project open` prints a
  heartbeat to stderr roughly every 15s (elapsed + last server log line);
  stdout stays JSON-only. `--timeout <seconds>` bounds the wait (default
  300s); on timeout with the process still alive the record is **kept** so
  the server stays reachable
- jadx passthrough args after `<file>` are normalized exactly like the
  TypeScript CLI: strip `--deobf`, inject `--show-bad-code`,
  `--no-imports`, `-Pdex-input.verify-checksum=no`, default
  `--rename-flags case,valid` (skipped when the user passed
  `--rename-flags`/`-rf`), and strip the `printable` token from rename-flag
  values so obfuscated Unicode identifiers survive decompilation
- When `--port` is omitted, `open` auto-assigns a free random port in
  `30000-40000`; `-P<key>=<value>` tokens after `<file>` forward to jadx-cli
- Output contract: JSON on stdout (`--format json|table`), notices/errors on
  stderr, and sysexits-style exit codes (`0` ok, `64` usage, `66` missing
  input/project, `69` server unavailable, `70` internal, `75` timeout);
  `DECX_DEBUG=1` enables HTTP/debug logging on stderr
- CLI data defaults to `~/.decx`; set `DECX_HOME` to redirect config,
  projects, logs, tmp downloads, tools.json, and installed server binaries
- `decx self install` installs or updates `decx-server.jar`; the
  skip-if-current check reads the version baked into the installed jar
  (`version.properties`) and prefers it over the config record. Release
  discovery uses the npm registry (stable) and the GitHub releases atom feed
  (prerelease) — no GitHub API calls
- Server/client transport is a std-only HTTP/1.1 client over TcpStream
  (`decx-cli-core::net`, Content-Length + chunked decoding, single-write
  requests); internet downloads delegate to the system `curl`
- ADB interaction is centralized in `decx-cli-core::tools::adb`
- `decx android device system-services` returns structured JSON for live
  Binder/system services and supports `--serial`, `--adb-path`, and `--grep`
- `decx android device permission-info <permission>` returns one structured
  JSON object for a permission and supports `--serial` and `--adb-path`
- Not ported from the TypeScript CLI yet (they fail with a clear
  `NOT_PORTED` error): the framework `collect`/`process`/`run` device-pull
  and image-extraction pipeline, and the `self skills` installer. The npm
  update-notifier is intentionally dropped

### Skill workflow details

- Skill architecture and authoring rules are defined in `skills/AGENTS.md`.
- Vulnerability hunting is the `decx-vulnhunt` skill with App and Framework tracks sharing one methodology, evidence gates, and rating authority; report/PoC skills consume its finalized finding writeups.
- `skills/decx-report/` (`decx-report`) owns report templates and consumes finalized DECX finding writeups; vuln-hunt skills should not duplicate report templates.
- PoC projects are generated by the agent on the spot: `skills/decx-poc/references/poc-base.md` is the single source of truth for the `poc-<target>/app/` + `poc-<target>/server/` contract; there are no template assets or setup scripts.
- The PoC app contract defined in `poc-base.md` keeps a dynamic button registry in `ExploitRegistry` and also accepts browser-driven `poc-<target>://run/trigger?exploit=<id>` launches through `PoCActivity`.

### Minimal OpenCode plugin

`.opencode/plugins/decx.js` is a minimal OpenCode plugin (auto-loaded from `.opencode/plugins/`) that only injects a routing hint into the system prompt, pointing the agent at the installed skills (`decx-cli`, `decx-vulnhunt`, `decx-report`, `decx-poc`). There is no graph database and no function-level tool set; workflow discipline is enforced by the skills themselves.

## Build And Test Commands

### Kotlin modules

```bash
cd decx
./gradlew dist
./gradlew :decx-plugin:shadowJar
./gradlew :decx-server:shadowJar
./gradlew test
```

Artifacts copied by Gradle:

- `decx/build/dist/jadx_decx_plugin-<version>.jar`
- `decx/build/dist/decx-server-<version>.jar`

Jadx script plugin: `jadx-script-kotlin` is not on Maven Central. `decx-server`'s `fetchJadxScriptPlugin` task downloads its GitHub release zip once and extracts the plugin jar; the scripting runtime (Kotlin scripting, ktlint, kotlin-logging) comes from Maven Central. Offline builds can set `DECX_JADX_SCRIPT_ZIP=/path/to/jadx-script-kotlin-<ver>.zip`. The fat jar uses Zip64 (>65535 entries) and its `META-INF/services/jadx.api.plugins.JadxPlugin` merge is verified to contain both `DexInputPlugin` and `JadxScriptKotlinPlugin`.

Version source:

- repository-root `version` file

### CLI (Rust)

```bash
cd decx-cli
cargo build --release   # target/release/decx[.exe]
cargo test
```

Rust 1.80+, edition 2021. The workspace has zero external C/TLS dependencies: the local-server transport is a hand-rolled HTTP/1.1 client over `std::net::TcpStream`, and internet downloads (self install, URL targets) delegate to the system `curl`. See `decx-cli/README.md` for the architecture.

## Technology And Style Notes

### Kotlin

- JVM toolchain: 17
- Main libraries: JADX, Javalin, Gson, Jackson, SLF4J/Logback
- Logging goes through `LogUtils`
- Error responses use `DecxError`
- Shared transport and routing live in `decx-core`; avoid duplicating server logic in plugin/server modules

Current error codes defined in `DecxError.kt` (see `decx/decx-core/src/main/kotlin/jadx/plugins/decx/api/DecxError.kt`):

- `INTERNAL_ERROR` (500), `SERVICE_ERROR` (503), `REQUEST_TIMEOUT` (504), `HEALTH_CHECK_FAILED` (500)
- `UNKNOWN_ENDPOINT` (404), `INVALID_PARAMETER` (400), `METHOD_NOT_FOUND` (404)
- `CLASS_NOT_FOUND` (404), `RESOURCE_NOT_FOUND` (404), `MANIFEST_NOT_FOUND` (404)
- `FIELD_NOT_FOUND` (404), `INTERFACE_NOT_FOUND` (404), `SERVICE_IMPL_NOT_FOUND` (404)
- `NO_STRINGS_FOUND` (404), `NO_MAIN_ACTIVITY` (404), `NO_APPLICATION` (404)
- `EMPTY_SEARCH_KEY` (400), `DECOMPILATION_SKIPPED` (503), `NOT_GUI_MODE` (503)

### Rust (decx-cli)

- Rust 1.80+ (edition 2021); workspace crates: `decx-cli-core` (library) + `decx` (binary)
- clap (builder API, default features off — color pulls in windows-sys, which needs dlltool on windows-gnu) for the command tree
- serde/serde_json for persistence and the wire format; sha2 for file hashing; flate2 for jar version.properties probing
- std-only HTTP/1.1 client for the local server; system `curl` for internet downloads
- `cargo test` (unit tests in-crate; integration points use DECX_HOME overrides, test artifacts under temp dirs)

### MCP server

- DECX exposes an in-process Kotlin MCP server (official `io.modelcontextprotocol:kotlin-sdk-server`) over Ktor CIO Streamable HTTP on `serverPort + 1` at `/mcp`.
- The MCP tool surface, transport, lifecycle, and registry live in `decx-core/.../server/`: `DecxMcpServer.kt`, `McpHttpServer.kt`, `McpToolRegistry.kt`.
- `McpToolRegistry` is backed by `DecxRoutes`; tools delegate to existing API routes, so MCP exposure stays in sync with HTTP exposure.
- MCP is **disabled by default**:
  - Standalone server: opt-in via `--mcp` (parsed in `DecxServerApp`)
  - CLI: `decx process open <file> --mcp` forwards the flag to `decx-server`
  - Plugin: auto-start driven by the `mcpAutoStart` preference (`PreferencesManager`)
- A `DecxApiResult` envelope is shared across HTTP and MCP responses; MCP tool responses are derived from the same `DecxApiResult` the HTTP layer returns.
- The Python MCP sidecar (`decx-plugin/src/main/resources/mcp/`) and its `SidecarProcessManager` / `McpPreferences` were removed in v3.4.0.

## Architecture Pointers

### Shared server path

For server behavior, follow this chain:

```text
DecxServer
  -> RouteHandler
  -> DecxApi / DecxApiImpl
  -> service/* and utils/*
```

Use this rule of thumb:

- New analysis capability usually starts in `DecxApi` and `DecxApiImpl`
- HTTP exposure is registered in `DecxRoutes`
- CLI exposure is added as a `Tool` implementation registered in `decx-cli-core::tools::ToolRegistry::with_builtins`
- MCP exposure is added in `decx-core/.../server/McpToolRegistry.kt`

### Plugin path

For plugin-only behavior, check:

- `DecxPlugin.kt`
- `lifecycle/PluginLifecycleManager.kt`
- `ui/DecxUIManager.kt`
- `utils/PreferencesManager.kt` (for `mcpAutoStart`)

### Standalone server path

For headless operation, check:

- `decx-server/src/main/kotlin/jadx/plugins/decx/server/DecxServerApp.kt`

This binary:

- parses `--port`
- parses `--mcp` (opt-in MCP server on `port + 1`)
- forwards remaining args to JADX CLI parsing
- validates the input file exists (and any `.jadx.kts` script files)
- defaults the log level to INFO (jadx-cli's PROGRESS mode sets root OFF, which would silence script `log` output); `--log-level` / `-q` / `-v` still override
- warms up the decompiler
- starts `DecxServer`

Jadx Kotlin scripts: pass `.jadx.kts` files as additional positional inputs (the CLI does this via `process open --script`). The bundled `jadx-script-kotlin` plugin evaluates them during `decompiler.load()` (top-level code) and registers `afterLoad` blocks as a `JadxAfterLoadPass`.

## Common Change Patterns

### Add or change an HTTP API endpoint

1. Add the capability in `DecxApi` and `DecxApiImpl`
2. Implement or extend logic in the relevant service under `decx-core/service/`
3. Register the route in `DecxRoutes`
4. If needed, update CLI and MCP consumers

### Add a CLI command

1. Add/extend a `Tool` (or subcommand inside one) under `decx-cli/crates/decx-cli-core/src/tools/`
2. Register it in `ToolRegistry::with_builtins`; the entrypoint assembles the tree from the registry
3. Add unit tests next to the code (pure helpers get direct tests; parsers get fixture tests)
4. Keep help text aligned with actual behavior, and keep the output contract: JSON stdout, stderr notices, sysexits exit codes

### Add an MCP tool

1. Update `decx/decx-core/src/main/kotlin/jadx/plugins/decx/server/McpToolRegistry.kt`
2. Point the tool at an existing `DecxRoutes` endpoint when possible
3. Only add new server APIs if the capability does not already exist

### Change plugin lifecycle or MCP startup

Validate interactions across:

- `PluginLifecycleManager`
- `DecxMcpServer` (in-process MCP server lifecycle)
- `PreferencesManager`
- `DecxUIManager`

Port coordination matters:

- DECX HTTP server uses the configured port
- Kotlin MCP server uses `port + 1`

## Key Files

| File | Why it matters |
|---|---|
| `AGENTS.md` | This repository guide for coding agents |
| `README.md` / `README_zh.md` | User-facing product and usage docs |
| `decx/settings.gradle.kts` | Gradle module inclusion |
| `decx/build.gradle.kts` | Root versioning, repositories, `dist` aggregation task |
| `decx/decx-core/src/main/kotlin/jadx/plugins/decx/Decx.kt` | Public facade for API, server, MCP, routes, tools |
| `decx/decx-core/src/main/kotlin/jadx/plugins/decx/server/DecxServer.kt` | Javalin HTTP server and route registration |
| `decx/decx-core/src/main/kotlin/jadx/plugins/decx/server/RouteHandler.kt` | Endpoint-to-API dispatch |
| `decx/decx-core/src/main/kotlin/jadx/plugins/decx/api/DecxApi.kt` | Shared API contract |
| `decx/decx-core/src/main/kotlin/jadx/plugins/decx/api/DecxApiImpl.kt` | Core API implementation |
| `decx/decx-core/src/main/kotlin/jadx/plugins/decx/api/DecxApiResult.kt` | Unified success/error envelope (HTTP + MCP) |
| `decx/decx-core/src/main/kotlin/jadx/plugins/decx/api/DecxError.kt` | Structured error codes |
| `decx/decx-core/src/main/kotlin/jadx/plugins/decx/server/DecxMcpServer.kt` | In-process Kotlin MCP server lifecycle |
| `decx/decx-core/src/main/kotlin/jadx/plugins/decx/server/McpHttpServer.kt` | Ktor CIO Streamable HTTP transport for MCP |
| `decx/decx-core/src/main/kotlin/jadx/plugins/decx/server/McpToolRegistry.kt` | MCP tool surface, backed by DecxRoutes |
| `decx/decx-core/src/main/kotlin/jadx/plugins/decx/utils/DecompileGuard.kt` | Single authority for decompiler-derived state: decompile guards, bounded code cache + cold-class unloading, class/method symbol index |
| `decx/decx-core/src/main/kotlin/jadx/plugins/decx/utils/BoundedCodeCache.kt` | Byte-bounded LRU `ICodeCache` installed by the headless server (JADX default is unbounded) |
| `decx/decx-core/src/main/kotlin/jadx/plugins/decx/utils/RouteTelemetry.kt` | In-flight + per-endpoint latency telemetry via `/health` and logs |
| `decx/decx-plugin/src/main/kotlin/jadx/plugins/decx/DecxPlugin.kt` | JADX plugin entry point |
| `decx/decx-plugin/src/main/kotlin/jadx/plugins/decx/lifecycle/PluginLifecycleManager.kt` | Startup sequencing |
| `decx/decx-plugin/src/main/kotlin/jadx/plugins/decx/ui/DecxUIManager.kt` | Plugin UI and restart actions |
| `decx/decx-server/src/main/kotlin/jadx/plugins/decx/server/DecxServerApp.kt` | Headless entry point |
| `decx-cli/crates/decx-cli-core/src/tools/mod.rs` | Tool trait, ToolRegistry, ToolContext (CLI assembly + dispatch) |
| `decx-cli/crates/decx-cli-core/src/tools/project_tool.rs` | Project lifecycle commands (open/close/list/status/check/watch/events) |
| `decx-cli/crates/decx-cli-core/src/project/manager.rs` | ProjectManager: records, probes, monitors, events |
| `decx-cli/crates/decx-cli-core/src/project/monitor.rs` | Background monitor threads (state machine + event stream) |
| `decx-cli/crates/decx-cli-core/src/engine/launcher.rs` | open flow: reuse decisions, detached spawn, health/exit wait |
| `decx-cli/crates/decx-cli-core/src/engine/foreign.rs` | External engine registry (`engines.json`, argv templates, PATH resolution) |
| `decx-cli/crates/decx-cli-core/src/tools/engine_tool.rs` | `engine register/query/list/show/remove` command group |
| `decx-cli/crates/decx-cli-core/src/tools/external.rs` | External CLI tool registry (opencli-style `tools register`) |
| `decx-cli/crates/decx-cli-core/src/client.rs` | DecxClient: all 26 analysis endpoints |

## Agent Guidance For This Repo

- Prefer updating `AGENTS.md` when repository behavior changes in ways that affect future coding agents.
- Keep this file grounded in code, not aspirational documentation.
- Avoid listing commands, endpoints, or scripts that are not actually present in the repo.
- When unsure whether user-facing behavior changed, verify against `README.md`, command sources, and Gradle/package manifests.
- If you add a new top-level module, new command group, or new transport path, update this file in the same change.
