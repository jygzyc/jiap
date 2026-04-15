# DECX v2.2.4

### Breaking Changes

- **Project renamed from JIAP to DECX** (Decompiler + X). This affects:
  - CLI command: `jiap` → `decx`
  - npm package: `jiap-cli` → `decx-cli`
  - Config directory: `~/.jiap/` → `~/.decx/`
  - Server JAR: `jiap-server.jar` → `decx-server.jar`
  - API paths: `/api/jiap/` → `/api/decx/`
  - Environment variables: `JIAP_*` → `DECX_*`
  - Kotlin package: `jadx.plugins.jiap` → `jadx.plugins.decx`
  - GitHub repository: `jygzyc/jiap` → `jygzyc/decx`
  - Skill names: `jiapcli` → `decxcli`

### Changes

- CLI: add `self install` and `self update` commands for server/CLI management.

- CLI: add `self install` and `self update` commands for server/CLI management.
- CLI: all commands output JSON by default, remove `--json` option.
- CLI: add CLI event and error logging to session log files (`~/.decx/logs/<session>.log`).
- CLI: add download progress bar for `decx-server.jar` installation.
- CLI: auto-sync server version from `/health` response on first API call.
- Server: expose version field in `/health` response.
- Server: move version.properties generation from decx-server to decx-core for unified version access.
- Skill (decxcli-poc): add `check-env.mjs` script for build environment detection.
- Skill (decxcli-poc): add subagent-based report verification step before writing PoC.
- Skill (decxcli-poc): defer compilation and env checks until user explicitly requests.
- Skill (decxcli-vulnhunt): keep decx session alive after analysis, mention PoC skill.
- Skill (decxcli): update command reference (self install/update, get-aidl path fix).

### Fixes

- CLI: fix `process open`/`close`/`status` commands producing no output (Commander not awaiting async action handlers).
- CLI: fix download progress label showing duplicate version (`vv2.2.1`).
- CLI: fix CLI version detection falling back to `unknown` when `npm_package_version` is unset.
- CLI: fix update check using string inequality instead of semver comparison.
- CLI: fix API call logging not capturing server error messages correctly.
- Server: fix AIDL implementation detection — replace `.Stub` with `$Stub` before replacing `.` with `/`.
