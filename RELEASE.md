# JIAP v1.1.1

## Performance Improvements

- **JADX Disk Cache**: Enable native JADX code caching to `~/.jiap/cache/` for faster repeated queries
- **Optimized Warmup**: Filter SDK packages and randomly sample app code during warmup, with full decompilation fallback for small apps
- **CodeInfo Caching**: Cache decompiled code info to reduce redundant decompilation

## Project Structure

- **Directory Restructure**: Rename `jiap_core/` to `jiap/` and move `mcp_server/` into `jiap/` directory for cleaner organization
- **Gradle Build**: Update build configuration for the new directory structure

## Documentation Enhancements

- **Refactored Audit Guides**: Reorganize security analysis documentation by component type (Activity, Service, ContentProvider)
- **Vulnerability Categorization**: Separate App vulnerabilities (IPC, WebView, SQL injection) from Framework vulnerabilities (Permission bypass, Identity confusion)
- **Standardized Tool APIs**: Update all tool signatures to use complete method signatures with named parameters
- **Taint Tracking Methodology**: Add comprehensive taint analysis workflow documentation

## Agent Skill Updates

**jiap-analyst**: 
- Component-specific analysis workflows for Activity, Service, and ContentProvider
- Framework-level vulnerability detection (Permission bypass, UID spoofing)
- Standardized tool API signatures matching MCP protocol