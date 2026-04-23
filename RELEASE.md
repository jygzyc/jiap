# DECX v3.2.0

DECX v3.2.0 unifies CLI filter parameter naming by merging `--first` and `--max-results` into a single `--limit` option.

### Changes

- CLI: `--first` and `--max-results` unified to `--limit` across `classes`, `search-global`, `search-class`, `class-source`, and other filter-bearing commands.

- Server: `DecxFilter` merges `first`/`maxResults` fields into `limit`, simplifying filter logic.

- MCP server: updated to match unified filter parameter naming.

- Docs: updated all README files, cli/README.md, and skill SKILL.md files with new command examples.
