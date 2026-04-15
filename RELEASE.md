# JIAP v2.2.1

### Changes

- CLI: move `get-aidl` from `code` to `ard` subcommand (Android-specific analysis).
- CI: use `gh api` with `--jq` for reliable prerelease cleanup instead of `gh release list` text parsing.

### Fixes

- API: fix get-aidl implementation detection — remove trailing semicolon in stubSmaliName that caused double-semicolon mismatch in smali `.super` lookup.
- API: rename `implementations` to `implements` in get-aidl response.
