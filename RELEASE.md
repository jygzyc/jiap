# JIAP v2.2.0

### Changes

- Server: add `get_aidl` API — automatically discover all AIDL interfaces in APK by scanning `$Stub` classes and their implementations via smali `.super` pattern matching.
- MCP: add `get_aidl` tool under Android Analysis endpoints.
- CLI: add `code get-aidl` command to list all AIDL interfaces and their implementations.
- CLI: default output to JSON format for `code` and `ard` commands (previously tree/YAML).
- CLI: file-based server ready detection with 120s timeout for `process open`.
- CLI: check port availability before starting server in `process open`.
- CLI: use esbuild define for version injection instead of runtime file read.
- CLI: Windows process spawn fixes and cross-platform logging improvements.
- Skill (vulnhunt): overhaul with subagent chain tracing architecture — structured 6-phase methodology with JSON input/output for chain subagents, forced command format enforcement, and template validation.
- Skill (vulnhunt): add comprehensive reference files for all vulnerability types (Activity, Service, Provider, Broadcast, WebView, Intent, Framework Service).
- Skill (vulnhunt): add report template and risk rating reference.
- Skill (poc): add `jiapcli-poc` skill with PoC construction methodology and per-component reference guides.
- Skill (poc): add `setup-poc.mjs` script for automated package name replacement in PoC projects.
- Skill (poc): repackage PoC template as zip with standardized project structure.
- Skill: quote all argument values in CLI command references to prevent shell parsing errors with method signatures.

### Fixes

- CLI: fix `receivers` command renamed to `app-receivers` in test expectations.
- Skill: fix incomplete method signatures in command references that could cause execution failures.

### Docs

- Update README and README_zh with `get_aidl` command in MCP tools, CLI commands.
- Update jiapcli and vulnhunt SKILL.md with `get-aidl` command.
