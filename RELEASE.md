# DECX v3.0.0

DECX v3.0.0 is a major release with a unified server API response format, new filter and search capabilities, service consolidation, and updated CLI commands and documentation.

### Changes

- Server: unified response format. All API endpoints now return `{ ok, kind, query, summary, items[], page }` replacing the old `{ success, data: { type, count, ...-list } }` envelope.

- Server: new `DecxError` enum with descriptive codes (`INTERNAL_ERROR`, `CLASS_NOT_FOUND`, `METHOD_NOT_FOUND`, etc.) and HTTP status codes, replacing the old `E001`-`E005` error codes.

- Server: new routing system with `DecxRoutes`, `DecxRequestParams` typed extractors, and `DecxFilter` (regex/literal includes/excludes with `first`/`maxResults` limits), replacing manual `when` dispatch.

- Server: service consolidation — `AndroidAppService` + `AndroidFrameworkService` merged into `AndroidService`; new `ContextService` for xref/inheritance; `VulnMiningService` removed.

- Server: new endpoints — `get_class_context`, `get_method_context` (callers + callees), `get_method_cfg` (control flow graph as DOT), `search_global_key` (cross-class search with filter).

- Server: `DecxApiResult.fail()` factory method for error results; `DecxFilter` with `Compiled` matcher for server-side filtering.

- CLI: new commands — `search-global <keyword> --max-results <n>`, `method-context <signature>`, `method-cfg <signature>`.

- CLI: `class-info` renamed to `class-context`; `search-class` now takes `<class> <pattern> --max-results <n>`.

- CLI: typed filter options (`--first`, `--include-package`, `--exclude-package`, `--no-regex`) on `all-classes`, `app-receivers`, `get-aidl`, `exported-components`, `search-global`, `search-class`.

- CLI: all filter-bearing commands support regex matching by default with `--no-regex` for literal text.

- Docs: updated error codes to `DecxError` enum; fixed all command references across README.md, cli/README.md, and all SKILL files; added package filtering recommendations to vulnhunt skill.
