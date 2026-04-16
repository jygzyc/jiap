# DECX v2.5.0

### Changes

- CLI: add `ard system-services` subcommand to list Android system services via `adb shell service list`, with `--grep` keyword filter.

- CLI: add `ard perm-info <permission>` subcommand to query permission details via `adb shell pm list permissions`.

- CLI: extend `AdbClient` with `listSystemServices()` and `getPermissionInfo()` methods, along with `parseSystemServicesOutput`, `filterSystemServices`, `parsePermissionInfoOutput`, and `buildPermissionInfoCommand` utility functions.

- CLI: refactor framework artifact metadata — remove legacy `.meta.json` mechanism, unify vendor identity on `.artifact.json` with automatic cleanup of stale meta files.

- CLI: fix `extractZipEntry` to use stream-based fd writing instead of buffering entire output in memory, preventing buffer overflow on large dex files.

- CLI: improve `self update` — read CLI package name dynamically from `package.json` instead of hardcoding `decx-cli`, and add `signal`/`error` handling for the npm install process.
