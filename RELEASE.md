# JIAP v2.1.0

### Changes

- Server: replace hardcoded JADX argument parsing with `JadxCLIArgs` passthrough — all standard jadx-cli options are now supported directly (e.g. `--deobf`, `--no-res`, `-j`, `--decompilation-mode`, `--rename-flags`, etc.).
- CLI: remove hardcoded JADX options from `process open`, transparently forward unknown args to jiap-server.
- CLI: migrate from pnpm to npm, clean up unused dev dependencies.
- CLI: read version from `package.json` instead of hardcoding.
- CLI: rename `receivers` command to `app-receivers` for consistency.
- CI (jiap): prerelease version now uses latest git tag + unix timestamp (e.g. `2.1.0-1718300400`).
- CI (jiap): fix prerelease cleanup to atomically delete releases and tags.
- CI (cli): use artifact sharing to avoid duplicate build in publish job.

### Fixes

- CLI: fix "too many arguments" error when passing jadx-cli options through `process open`.
- CLI: fix nested `~/.jiap/bin/bin/` directory creation in installer.

### Docs

- Update README and SKILL.md to reflect npm install and jadx option passthrough.
