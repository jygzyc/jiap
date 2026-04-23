# DECX v3.1.0

DECX v3.1.0 adds resource and source filtering, session management improvements, and unified CLI error handling.

### Changes

- CLI: `all-classes` renamed to `classes` across all commands, API endpoints, MCP server, and skill files.

- CLI: `class-source` now supports `--first <n>` to return only the first N source lines, reducing output for large classes.

- CLI: `all-resources` now supports `--include <pattern> [--no-regex]` for file-name filtering.

- CLI: `process close` now supports `--port <port>` to stop a session by port number instead of name.

- CLI: unified error handling — all command actions wrapped with `withErrorHandler` for consistent error output.

- CLI: new `resolveCommandClient` helper that merges global options from parent commands, fixing option resolution in subcommands.

- Server: `get_all_classes` endpoint renamed to `get_classes`.

- Server: `get_class_source` accepts `first` filter parameter for source truncation.

- Server: `get_all_resources` accepts `includes` and `regex` filter parameters for resource filtering.

- MCP server: updated to match renamed and filtered endpoints.

- Docs: updated all README files, cli/README.md, and skill SKILL.md files to reflect new commands and options.
