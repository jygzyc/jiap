# AGENTS.md

Coding agent instructions for the DECX repository.

## Repository Purpose

DECX (`Decompiler + X`) is an AI-oriented analysis layer built on top of JADX.
The repository contains:

- A Kotlin HTTP analysis server shared by plugin mode and standalone mode
- A JADX GUI plugin that starts the DECX server and exposes its settings UI
- A standalone `decx-server` fat JAR for headless analysis
- A TypeScript CLI that starts and talks to `decx-server`
- AI skill definitions under `skills/` for DECX-driven analysis workflows

Primary request flow:

```text
AI Assistant / CLI
  -> direct HTTP
  -> DECX HTTP server
  -> DecxApi
  -> JADX decompiler state
```

## Repository Layout

| Path | Stack | Role |
|---|---|---|
| `decx/decx-core/` | Kotlin, JVM 17 | Shared API, HTTP transport, services, models, utilities |
| `decx/decx-plugin/` | Kotlin, Shadow JAR | JADX GUI plugin, lifecycle, UI |
| `decx/decx-server/` | Kotlin, Shadow JAR | Standalone headless server with `DecxServerApp` main class |
| `decx-cli/` | TypeScript, Node.js 22.5+ | User CLI for session management and analysis commands |
| `decx-native/` | Rust, pure std | Native engine workspace: decx json/apk/core/taint crates, `decx-engine` (in-house decompiler core adapted from dexdec/rusty-dex), `decx-native-server` (same 26-endpoint contract as the JVM server, plus the native-only `taint_scan`) |
| `skills/decx-cli/` | Skill `decx-cli` | DECX CLI usage, general analysis, and workflow routing |
| `skills/decx-vulnhunt/` | Skill `decx-vulnhunt` | Android vulnerability hunting workflow (App + Framework tracks) |
| `skills/decx-report/` | Skill `decx-report` | Report generation from finalized DECX analysis graph findings |
| `skills/decx-poc/` | Skill `decx-poc` | PoC app construction workflow |

## What Is Actually Implemented

### Kotlin server capabilities

`decx-core` exposes these HTTP endpoints through `DecxRoutes` and `RouteHandler` (the native Rust engine in `decx-native/` exposes the same endpoint set through `decx-native/crates/decx-server/src/routes.rs`):

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

### Native Rust engine (`decx-native`)

`decx-native/` is a standalone zero-external-dependency (pure std) workspace — the native engine:

- `crates/decx-engine` — the in-house source-level decompiler core (`decx-engine`/`decx_engine`), adapted from two Apache-2.0 upstreams with per-file attribution (see the next bullet). It consolidates the former `crates/dexdec` + `crates/rusty-dex` + `crates/shims/*`: `src/rusty_dex/` is the DEX parser module (from `rusty-dex`, zips read through `decx-apk`, `thiserror`/`regex`/`lazy_static`/`byteorder` hand-rewritten on std), and `src/{log,rayon,zstd,crc32fast}.rs` are original sequential no-op dependency-shim modules (same names so adapted `use` lines stay close to upstream). `symbol-builder`/`cli` features stay off; `resources/symbols/platform.dexsym` is vendored and embedded (repacked uncompressed — the `zstd` shim is an identity passthrough).
- Adapted-source attribution rule: every `.rs` file under `crates/decx-engine/src` except the four shim modules carries a provenance header (upstream project, version, Apache-2.0, and whether the file is byte-identical to upstream or MODIFIED — Apache-2.0 §4(b)). Provenance tables, baseline commit (`asLody/dexdec` tag `v1.0.2`, commit `6c083d9`), the full modified-file lists, and re-diff instructions live in `crates/decx-engine/VENDORED.md`. When editing an adapted file, keep its header accurate and mark new changes in-line with a `decx-native:` comment. `tools/stamp-vendor-headers.sh` (re)applies headers against an upstream checkout; `tools/verify-vendor-headers.sh` re-derives the classification and reports header/content mismatches.
- `crates/decx-core` keeps the lightweight dex parser (`dex.rs`, `opcodes.rs`, `leb128.rs`) + global xref index (`xref.rs`), structural Java emitter (`java.rs`, `code.rs`), and `sources.rs`: source endpoints (`get_class_source`, `get_method_source`, `get_system_service_impl`) prefer the decx-engine pipeline and fall back to the structural-IR emitter, reporting `meta.mode` = `dexdec` | `structural-ir` (the `dexdec` value is kept as the wire/API contract name); `regex.rs` is a hand-rolled regex engine used by the filter-enabled endpoints
- `crates/decx-apk` (zip/inflate/AXML/manifest), `crates/decx-json` (insertion-ordered JSON), `crates/decx-taint` (Mariana-Trench-style inter-procedural taint solver: rule model in `model.rs` embedded from `resources/taint-rules.json`, method-body lowering in `body.rs`, intra-procedural analysis + method summaries in `intra.rs`, fixpoint driver in `solver.rs`, virtual-dispatch same-key fan-out in `dispatch.rs`), `crates/decx-server` (hand-rolled HTTP/1.1 in `http.rs`, 26-endpoint + native-only `taint_scan` dispatch in `routes.rs`, DecxApiResult-compatible envelope in `envelope.rs`).
- `decx-native-server` binary (`crates/decx-server/src/main.rs`): `<target.apk|classes.dex|xx.jar> [--port N] [--warm] [--taint-rules <rules.json>]`, `--port` default 25419, `--taint-rules` (or `DECX_TAINT_RULES` env) replaces the embedded taint rule set at startup and fail-fasts on unreadable/invalid files (rules support `"param": <n>` to anchor a callback's declared parameter as a source), Kotlin-compatible `/health` with cache counters. Method-source requests carry the full `Lcls;->name(args)ret` key; the descriptor disambiguates same-name overloads (`MethodRequest.descriptor`), ambiguous methods report `METHOD_NOT_FOUND` with the candidate descriptors
- Cross-platform: pure Rust, no `cfg(unix)`/`cfg(windows)` branches, builds on Windows/Linux/macOS. The `release-native` job in `.github/workflows/release-decx.yml` builds and attaches `decx-native-server-<version>-<rust-target>[.exe]` for `x86_64-pc-windows-msvc`, `x86_64-unknown-linux-gnu`, `aarch64-apple-darwin`, and `x86_64-apple-darwin` (cross-compiled from the arm64 macOS runner) on every `v*` tag, running `cargo test --release -p decx-core` on each platform first
- Known divergences from the JVM engine: `--script` is JVM-only (the CLI fails fast for native); `smali=true` output comes from the structural-IR emitter (not real smali); Kotlin rendering (`language=kotlin|auto` on the source endpoints) is native-only — the JVM engine ignores the field and always serves Java; `POST /api/decx/taint_scan` is native-only (Mariana-Trench-style inter-procedural data-flow scan with sources/sinks/propagators/sanitizers rules embedded in `crates/decx-taint/resources/taint-rules.json`, replaceable at startup via `--taint-rules`/`DECX_TAINT_RULES`; sources support `"param"` anchoring for callback entries, virtual/interface/super/polymorphic invokes fan out over same-key overrides while sanitizers/propagators stay literal; request takes optional `maxRounds`/`maxFindings`, response items carry `source`/`sink`/`route`/`severity` and the envelope summary carries `rounds_run`/`methods_analyzed`/`methods_total`/`static_fields_tracked`/`truncated`). The TypeScript `decx-cli` is the only client (no Rust CLI); it drives the native engine via `--engine native`

### CLI engine selection

- `decx process open <file> --engine native` spawns `decx-native-server` instead of the JVM jar; `--engine jvm` (default) keeps the current behavior; `DECX_ENGINE=native` sets the default
- Native binary discovery (`findDecxNativeServer` in `decx-cli/src/core/installer.ts`): `DECX_NATIVE_SERVER` env (file or dir) > `DECX_HOME/bin/decx-native-server[.exe]` > dev checkout `decx-native/target/release`
- Session reuse requires matching engine; a live session on the same file with a different engine is an error unless `--force`
- `--script` with `--engine native` fails fast with a clear message; jadx passthrough flags are ignored for native spawns
- `process check` reports both the jar and the native binary status

### Plugin responsibilities

The JADX plugin does more than just expose the server:

- Waits until the decompiler is ready before creating DECX services
- Initializes preferences and server port
- Starts the embedded DECX HTTP server
- Provides UI and restart hooks through `DecxUIManager`
- Bounds decompiler memory on headless servers: a byte-capped LRU code cache (`decx.decompile.cacheMaxBytes` → default `min(4G, -Xmx/2)`) plus a backpressured daemon that unloads evicted classes; see `DecompileGuard`

### CLI responsibilities

The CLI is session-oriented and can spawn standalone DECX server processes.
Current top-level commands are:

- `decx process`
- `decx code`
- `decx android`
- `decx self`

Notable details:

- `decx process open <file>` launches `java -jar decx-server.jar ...`
- `decx process open <file>` starts the JVM with `-Xmx` set to 2/3 of machine memory rounded down
- `decx process open <file>` is also reused by `decx android framework open` and `decx android framework run`
- `decx process open <file> --script <s1.jadx.kts> [--script <s2.jadx.kts> ...]` runs Jadx Kotlin scripts during decompilation; scripts are passed to decx-server as positional input files after the main target
- Scripts execute at decompile time (top-level code at load, `jadx.afterLoad { }` blocks after classes load); the server bundles the `jadx-script-kotlin` plugin
- Session reuse is keyed on the target file **plus** the exact script set; opening the same file with a different script set errors until `--force`
- `--force` replaces alive sessions matching the same name **or** the same file hash: their JVMs are killed with verified death before the new server starts. A failed kill aborts the spawn (session record kept, pid reported) instead of leaking orphan processes; `process close` keeps the record on failed kills too
- While waiting for the server to become healthy, `process open` prints a heartbeat to stderr roughly every 15s (elapsed time + last server log line); stdout stays JSON-only
- `process open --timeout <seconds>` bounds the health wait (default 300s). On timeout with the JVM still alive, the session record is **kept** and the error suggests `decx process check --port <port>` / `decx process close`; the record is only removed when the JVM exited
- Standard `jadx-cli` flags are passed through by `process open`
- `process open` auto-injects `--show-bad-code`, `--no-imports`, and `-Pdex-input.verify-checksum=no` (each skipped if already present), and intentionally strips `--deobf` because DECX relies on original symbol names
- `process open` also injects `--rename-flags case,valid` by default (skipped when the user passed `--rename-flags`/`-rf` in any form) and strips the `printable` token from user-supplied rename-flag values: jadx's default `printable` rename replaces non-ASCII obfuscated identifiers (e.g. `Ď锬볝觧`) with `m0`-style aliases in decompiled source, which breaks DECX's original-name contract (`all` is rewritten to `case,valid`; `none` and unparseable values pass through untouched)
- No DECX command binds `-P` to `--port`; `-P<key>=<value>` tokens are forwarded to jadx-cli by `process open` as JADX project properties. Use `--port` everywhere for the server port
- When `--port` is omitted, `process open` auto-assigns a free random port in `30000–40000` (checked for availability, retried on collision); the chosen port is recorded on the session
- CLI sessions are tracked locally and can be reused by session name and file hash
- `decx process close` can close by session name, by `--port <port>`, or all sessions with `--all`
- CLI data defaults to `~/.decx`; set `DECX_HOME` to redirect config, sessions, logs, tmp files, output, and installed server JARs
- CLI tests set `DECX_HOME` to `.decx_test/home/.decx` and keep test-only artifacts under `.decx_test/`
- `decx self install` installs or updates `decx-server.jar`; the skip-if-current check reads the version baked into the installed jar (`version.properties`) and prefers it over the config record, so stale records or manually replaced jars are handled correctly
- `decx self skills install --client <client>` downloads DECX skills from GitHub into `DECX_HOME/skills`, then symlinks them into private directories for Codex, Claude Code, and Cursor or the shared `~/.agents/skills` directory for every other or omitted client
- `decx self update` updates both the server JAR and the currently installed npm CLI package
- On startup the CLI runs a non-blocking update check (`decx-cli/src/core/update-notifier.ts`): the latest version comes from the npm registry, results are cached in `DECX_HOME/update-check.json` for 24 hours, refreshes happen in a detached `__update-check` child process, and update hints go to stderr; disable with `DECX_NO_UPDATE_CHECK=1` (also skipped under `CI`)
- Framework processing is implemented in native TypeScript under `decx-cli/src/android/`
- `decx-cli` builds runtime JavaScript as two bundles: `dist/index.js` for the CLI and `dist/sdk/index.js` for SDK imports; packaged native tools are stored as `dist/bin.tar.gz`
- Packaged native tools are extracted to `DECX_HOME/bin` (next to `decx-server.jar`), gated by a `.native-tools.sha256` content-hash marker that cleans and re-extracts on upgrade
- `decx android framework` provides framework collection and preprocessing subcommands:
  `collect`, `process`, `run`, `open`
- `decx android device` provides adb-backed inspection commands:
  `system-services`, `permission-info`
- Framework collection is tiered: ready-made files first (`/system/framework`, the runtime `/apex` mount whose activated modules expose already-extracted `javalib` jars, `/vendor/framework`, `/system_ext/framework`), then `.apex`/`.capex` images from `/system/apex` only for modules `/apex` did not already cover (result field `skippedCoveredModules`). At process time, jars/dex under a source `apex/<module>/...` layout reuse the APEX post-extraction scheme (`<module>_`-prefixed dex outputs, `@version` dir suffixes stripped); `.apex` files keep going through payload-image extraction
- Zip/jar read-write operations are centralized in `decx-cli/src/android/zip-utils.ts` and are cross-platform: Windows 10+ uses the bundled bsdtar (`C:\Windows\System32\tar.exe`, no `zip`/`unzip` dependency), other platforms use Info-ZIP `zip`/`unzip`
- ext4 `apex_payload.img` images are parsed natively in TypeScript (`decx-cli/src/android/ext4-reader.ts`: superblock → group descriptors → extents → dirents, jar/apk/dex extracted without any external tool); EROFS payloads and unsupported ext4 features fall back to external tools. Framework APEX image-extraction fallback tools (debugfs for unsupported ext4 features, erofs-utils for EROFS payloads) have no native Windows binaries; only `extract.erofs`/`fsck.erofs` are packaged (no packaged debugfs — the native TS ext4 reader covers ext4 payloads; system/WSL e2fsprogs serves as the rare fallback). On Windows `decx-cli/src/android/framework-tools.ts` delegates those tools to WSL (`wsl.exe`) with `/mnt/<drive>/...` path translation (`translateWslArgs`), falling back to the packaged `linux/x86_64/extract.erofs`. WSL tools are exec'd via absolute distro paths resolved with `command -v` (never bare names): some WSL relay builds fail bare-name exec of `/usr/sbin` binaries with `execvpe(...) failed: No such file or directory` even when the binary is installed. Tools are resolved lazily during `framework process` — only when a payload actually needs them — so ext4-only and `/apex`-pulled sources work without WSL
- ADB interaction is centralized in `decx-cli/src/android/adb.ts`
- `decx android device system-services` returns structured JSON for live Binder/system services and supports `--serial`, `--adb-path`, and `--grep`
- `decx android device permission-info <permission>` returns one structured JSON object for a permission and supports `--serial` and `--adb-path`
- `get_classes` accepts a `filter` object with `limit`, regex-enabled `includes`/`excludes`, and optional `regex=false`
- `get_class_source` accepts an optional `filter.limit` to return at most N source lines, plus an optional `language` field (`java` | `kotlin` | `auto`; default `java`): `kotlin` renders through the native engine's decx-engine Kotlin backend, `auto` infers per class from `kotlin.Metadata`/source-file name. `get_method_source` accepts the same field. The JVM engine ignores the field and always serves Java; the native engine labels the actual language in item `meta.language` and caches Kotlin renderings under separate keys
- `decx code class-source` / `decx code method-source` expose this as `--language <java|kotlin|auto>` (validated client-side; omitted keeps the request body unchanged for JVM sessions)
- `get_aidl_interfaces` and `get_dynamic_receivers` accept the same regex-enabled `filter` object for package filtering
- `get_exported_components` accepts regex-enabled `includes`/`excludes` and optional `regex=false`
- `get_all_resources` accepts `filter.includes` and optional `regex=false` for resource file-name filtering
- `search_global_key` accepts a `search` object with `limit`, `includes`, `excludes`, `caseSensitive`, and `regex`
- `search_class_key` greps within one class and requires a `grep` object with `limit`, `caseSensitive`, and `regex`
- Framework build metadata is stored per-output-directory under `.artifact.json`; legacy `.meta.json` is no longer used. The artifact vendor (device model) is auto-detected: a single connected adb device is auto-selected; several devices require `--serial` (`ADB_DEVICE_AMBIGUOUS`); no device keeps the offline `unknown` default
- `decx android framework open` / `run` ultimately create normal process sessions via `decx process open`; framework artifacts are not stored as a separate session kind
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

### Native Rust engine

```bash
cd decx-native
cargo build --release   # produces target/release/decx-native-server[.exe]
cargo test              # dexdec 618 + decx-core + decx-engine suites
```

The workspace has zero external dependencies (offline by construction; no vendored registry needed). Jadx Kotlin scripts are JVM-engine only.

Version source:

- repository-root `version` file

### CLI

```bash
cd decx-cli
npm install
npm run build
npm test
npm run lint
npm run typecheck
npm run dev
```
`npm run build` type-checks (via `tsc --noEmit` behind the build script) and emits a compact runtime bundle under `dist/`. `npm run typecheck` runs `tsc --noEmit` standalone for CI/local checks.

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

### TypeScript

- ESM project (`"type": "module"`)
- Commander-based command tree
- esbuild-based bundle
- Jest-based tests
- Node.js requirement: `>=22.5`

### MCP support (removed)

MCP support was removed from the server stack; the DECX HTTP API is the only
interface: `POST /api/decx/<endpoint>` with the `DecxApiResult` envelope
(plus `GET /health`). AI clients integrate through that surface directly -
the TypeScript `decx-cli` is an HTTP client, and the skills drive the CLI.
`decx-server` rejects `--mcp`/`--no-mcp` with an explicit error. The Python
MCP sidecar (`decx-plugin/src/main/resources/mcp/`) was removed earlier, in
v3.4.0; the Kotlin MCP server (`DecxMcpServer` / `McpHttpServer` /
`McpToolRegistry`, Ktor transport on `port + 1`) is removed in this change.

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
- CLI exposure is added in `decx-cli/src/commands/`

### Plugin path

For plugin-only behavior, check:

- `DecxPlugin.kt`
- `lifecycle/PluginLifecycleManager.kt`
- `ui/DecxUIManager.kt`
- `utils/PreferencesManager.kt` (server port, cache mode)

### Standalone server path

For headless operation, check:

- `decx-server/src/main/kotlin/jadx/plugins/decx/server/DecxServerApp.kt`

This binary:

- parses `--port`
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
4. If needed, update CLI consumers

### Add a CLI command

1. Extend the relevant file in `decx-cli/src/commands/`
2. If it is a new command group, register it in `decx-cli/src/index.ts`
3. Add or update tests in `decx-cli/tests/`
4. Keep help text aligned with actual behavior

### Change plugin lifecycle

Validate interactions across:

- `PluginLifecycleManager`
- `PreferencesManager`
- `DecxUIManager`

The DECX HTTP server uses the configured port; there is no secondary MCP port.

## Key Files

| File | Why it matters |
|---|---|
| `AGENTS.md` | This repository guide for coding agents |
| `README.md` / `README_zh.md` | User-facing product and usage docs |
| `decx/settings.gradle.kts` | Gradle module inclusion |
| `decx/build.gradle.kts` | Root versioning, repositories, `dist` aggregation task |
| `decx/decx-core/src/main/kotlin/jadx/plugins/decx/Decx.kt` | Public facade for API, server, routes |
| `decx/decx-core/src/main/kotlin/jadx/plugins/decx/server/DecxServer.kt` | Javalin HTTP server and route registration |
| `decx/decx-core/src/main/kotlin/jadx/plugins/decx/server/RouteHandler.kt` | Endpoint-to-API dispatch |
| `decx/decx-core/src/main/kotlin/jadx/plugins/decx/api/DecxApi.kt` | Shared API contract |
| `decx/decx-core/src/main/kotlin/jadx/plugins/decx/api/DecxApiImpl.kt` | Core API implementation |
| `decx/decx-core/src/main/kotlin/jadx/plugins/decx/api/DecxApiResult.kt` | Unified success/error envelope (HTTP) |
| `decx/decx-core/src/main/kotlin/jadx/plugins/decx/api/DecxError.kt` | Structured error codes |
| `decx/decx-core/src/main/kotlin/jadx/plugins/decx/utils/DecompileGuard.kt` | Single authority for decompiler-derived state: decompile guards, bounded code cache + cold-class unloading, class/method symbol index |
| `decx/decx-core/src/main/kotlin/jadx/plugins/decx/utils/BoundedCodeCache.kt` | Byte-bounded LRU `ICodeCache` installed by the headless server (JADX default is unbounded) |
| `decx/decx-core/src/main/kotlin/jadx/plugins/decx/utils/RouteTelemetry.kt` | In-flight + per-endpoint latency telemetry via `/health` and logs |
| `decx/decx-plugin/src/main/kotlin/jadx/plugins/decx/DecxPlugin.kt` | JADX plugin entry point |
| `decx/decx-plugin/src/main/kotlin/jadx/plugins/decx/lifecycle/PluginLifecycleManager.kt` | Startup sequencing |
| `decx/decx-plugin/src/main/kotlin/jadx/plugins/decx/ui/DecxUIManager.kt` | Plugin UI and restart actions |
| `decx/decx-server/src/main/kotlin/jadx/plugins/decx/server/DecxServerApp.kt` | Headless entry point |
| `decx-cli/src/index.ts` | CLI command registration |
| `decx-cli/src/commands/process.ts` | Session lifecycle and server spawning |
| `decx-cli/src/commands/code.ts` | Common code-analysis commands |
| `decx-cli/src/commands/android.ts` | Android-analysis commands |
| `decx-cli/src/commands/self.ts` | CLI/server self-management |
| `decx-native/crates/decx-server/src/routes.rs` | Native engine: all-endpoint dispatcher with DecxApiResult envelope |
| `decx-native/crates/decx-server/src/envelope.rs` | Native engine: success/error envelope + pagination (Kotlin-compatible) |
| `decx-native/crates/decx-core/src/sources.rs` | Native engine: decx-engine source-recovery bridge with structural-IR fallback |
| `decx-native/crates/decx-core/src/project.rs` | Native engine: dex/apk/jar loading, class index, decompiler state |
| `decx-native/crates/decx-apk/src/axml.rs` | Native engine: binary AndroidManifest.xml (AXML) decoder |
| `decx-native/crates/decx-apk/src/manifest.rs` | Native engine: APK manifest component model |
| `decx-native/crates/decx-engine/VENDORED.md` | Native engine: adapted-source provenance + modification policy |
| `decx-native/crates/decx-server/src/main.rs` | `decx-native-server` HTTP server (Kotlin-compatible health + errors) |

## Agent Guidance For This Repo

- Prefer updating `AGENTS.md` when repository behavior changes in ways that affect future coding agents.
- Keep this file grounded in code, not aspirational documentation.
- Avoid listing commands, endpoints, or scripts that are not actually present in the repo.
- When unsure whether user-facing behavior changed, verify against `README.md`, command sources, and Gradle/package manifests.
- If you add a new top-level module, new command group, or new transport path, update this file in the same change.
