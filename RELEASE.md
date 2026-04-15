# JIAP v2.2.3

### Changes

- CLI: add `self install` and `self update` commands for server/CLI management.
- CLI: all commands output JSON by default, remove `--json` option.
- CLI: add CLI event and error logging to session log files (`~/.jiap/logs/<session>.log`).
- CLI: add download progress bar for `jiap-server.jar` installation.
- CLI: auto-sync server version from `/health` response on first API call.
- Server: expose version field in `/health` response.
- Server: move version.properties generation from jiap-server to jiap-core for unified version access.
- Skill (jiapcli-poc): add `check-env.mjs` script for build environment detection.
- Skill (jiapcli-poc): add subagent-based report verification step before writing PoC.
- Skill (jiapcli-poc): defer compilation and env checks until user explicitly requests.
- Skill (jiapcli-vulnhunt): keep jiap session alive after analysis, mention PoC skill.
- Skill (jiapcli): update command reference (self install/update, get-aidl path fix).

### Fixes

- CLI: fix `process open`/`close`/`status` commands producing no output (Commander not awaiting async action handlers).
- CLI: fix download progress label showing duplicate version (`vv2.2.1`).
- CLI: fix CLI version detection falling back to `unknown` when `npm_package_version` is unset.
- CLI: fix update check using string inequality instead of semver comparison.
- CLI: fix API call logging not capturing server error messages correctly.
- Server: fix AIDL implementation detection — replace `.Stub` with `$Stub` before replacing `.` with `/`.
