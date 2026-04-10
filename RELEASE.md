# JIAP v2.0.0

### Changes

- Architecture: split monolithic `jiap` into `jiap-core`, `jiap-plugin`, `jiap-server` multi-module Gradle projects for cleaner separation of API, plugin UI, and headless server.
- CLI: add standalone TypeScript CLI (`jiap process` / `jiap ard` / `jiap code`) with session management, typed API client, and CI build pipeline.
- Session: replace PID-based sessions with name-based sessions for better stability and persistence across process restarts.
- API: add resource analysis endpoints for asset extraction, field cross-references, exported components, deep links, and dynamic receivers.
- Skill: replace legacy `jiap-analyst` with `jiapcli` core skill and `jiapcli-vulnhunt` hunting skill, including 51 categorized vulnerability references.
- Runtime: raise minimum Java version to 11, add MCP auto-start config, and simplify plugin UI.
- Build: add GitHub Actions workflow for CLI (`build-cli.yml`) and multi-module JAR packaging.

### Fixes

- Performance: eliminate double hash computation on APK open.
- Dedup: add hash check to prevent analyzing the same APK under different names.

### Removed

- `preprocessor/` directory and `docs/` directory.
- Legacy `skills/jiap-analyst/` skill.
