# DECX - Decompiler + X

<div align="center">

![DECX Logo](https://img.shields.io/badge/DECX-Decompiler%20%2B%20X-blue?style=for-the-badge&logo=java&logoColor=white)
![Release](https://img.shields.io/github/v/release/jygzyc/decx?style=for-the-badge&logo=github&color=green)
![License](https://img.shields.io/github/license/jygzyc/decx?style=for-the-badge&logo=gnu&color=orange)

**A JADX-based Decompiler + X - Designed for AI-assisted code analysis**

</div>

---

## Overview

DECX (Decompiler + X) is a smart code analysis platform built on the JADX decompiler, designed specifically for AI-assisted code analysis. The platform provides powerful Java code analysis capabilities to AI assistants through HTTP API and MCP (Model Context Protocol).

---

## Quick Start

### Prerequisites

- **Java**: JDK 11+ (for DECX Core)
- **Python**: 3.10+ (optional - auto-managed)
- **JADX**: v1.5.2+ with plugin support
- **Python dependencies**: `requests`, `fastmcp`, `pydantic` (auto-installed)

### Installation

```bash
# 1. Install plugin in JADX GUI
# JADX -> Settings -> Plugins -> Install DECX

# Or install via command line
jadx plugins --install-jar <path-to-decx.jar>

# 2. Build DECX Core (from source)
cd decx
chmod +x gradlew
./gradlew dist
```

**MCP Server Auto-Management:**
- Plugin automatically extracts and manages MCP server scripts
- No manual Python dependency installation or MCP server startup required
- No manual environment variable configuration required

### Usage

* Start DECX Plugin

  - Launch JADX and enable the DECX plugin
  - The plugin automatically starts the HTTP server, MCP server can be manually confirmed to start
  - Verify the server is running at `http://127.0.0.1:25419`

**Automatic Process:**
1. DECX plugin starts HTTP server (port `25419`)
2. Plugin extracts MCP scripts to `~/.decx/mcp/`
3. If auto-start is enabled, companion process (MCP server) starts automatically (port `25419 + 1`)
4. Both processes stop together on shutdown

* Verify Connection

Use `health_check()` to verify the connection between MCP server and DECX plugin

* Available Tools

All tools support pagination via the `page` parameter.

**Code Analysis**
- `get_all_classes(first=None, include_packages=None, exclude_packages=None, page=1)` - Retrieve classes with optional package filtering
- `search_global_key(key, first=None, max_results, include_packages=None, exclude_packages=None, case_sensitive=False, regex=True, page=1)` - Search all class bodies with package and result limits
- `search_class_key(class_name, key, max_results, case_sensitive=False, regex=True, page=1)` - Grep one class and return matching lines with method signatures
- `get_class_source(class_name, smali=False, page=1)` - Get class source code in Java or Smali format
- `search_method(method_name, page=1)` - Search for methods matching the name
- `get_method_source(method_name, smali=False, page=1)` - Get method source code
- `get_class_context(class_name, page=1)` - Get class information (fields and methods)
- `get_method_context(method_name, page=1)` - Get method signature, callers, and callees
- `get_method_cfg(method_name, page=1)` - Get method control flow graph as DOT source
- `get_method_xref(method_name, page=1)` - Find method usage locations
- `get_field_xref(field_name, page=1)` - Find field usage locations
- `get_class_xref(class_name, page=1)` - Find class usage locations
- `get_implement(interface_name, page=1)` - Get interface implementations
- `get_sub_classes(class_name, page=1)` - Get subclasses

**UI Integration**
- `selected_text(page=1)` - Get currently selected text in JADX GUI
- `selected_class(page=1)` - Get currently selected class in JADX GUI

**Android Analysis**
- `get_app_manifest(page=1)` - Get Android manifest content
- `get_main_activity(page=1)` - Get main activity source
- `get_application(page=1)` - Get Android application class and information
- `get_exported_components(component_types=None, regex=True, page=1)` - Get exported components with optional regex type filtering
- `get_deep_links(page=1)` - Get app URL schemes and intent filters
- `get_all_resources(page=1)` - List all resource file names (including resources.arsc sub-files)
- `get_resource_file(resource_name, page=1)` - Get resource file content by name
- `get_strings(page=1)` - Get strings.xml content from app resources
- `get_dynamic_receivers(first=None, include_packages=None, exclude_packages=None, regex=True, page=1)` - Get dynamically registered BroadcastReceivers with package filters
- `get_aidl(first=None, include_packages=None, exclude_packages=None, regex=True, page=1)` - Get AIDL interfaces and implementations with package filters
- `get_system_service_impl(interface_name, page=1)` - Get system service implementations

**System**
- `health_check()` - Verify server connection status

### Configuration

**Port Configuration:**
- **GUI**: DECX Server Status menu → Set new port → Auto-restart
- **Plugin Options**: Set `decx.port` in JADX plugin options
- **Default**: `25419` (Decx)

**MCP Script Path:**
- **GUI**: DECX Server Status menu → Browse and select custom script
- **Plugin Options**: Set `decx.mcp_path` to custom script path
- **Default**: Auto-extracted to `~/.decx/mcp/decx_mcp_server.py`

**Companion Process Configuration:**
```bash
# Auto-detected executor: uv, python3, or python
# Auto-extracted scripts to ~/.decx/mcp/
# Auto-started with correct DECX_URL and MCP_PORT
```

**Cache Configuration:**
DECX supports two cache modes for improved performance:
- **disk** (default): Persists decompilation cache to disk (`~/.decx/cache/`)
- **memory**: Keeps cache in memory only, suitable for small projects

**Configuration:**
- **Plugin Options**: Set `decx.cache` to `disk` or `memory`
- **Default**: `disk` for better performance on subsequent runs

**Performance Optimization:**
DECX includes automatic performance optimizations:

**Decompiler Warmup:**
- DECX automatically warms up the decompiler engine on startup
- Filters out SDK packages (android.*, androidx.*, java.*, javax.*, kotlin.*)
- Randomly samples up to 15,000 application classes
- Ensures optimal performance for subsequent queries

**Disk Caching:**
- Decompiled code is cached to disk for faster retrieval
- Cache persists across JADX sessions
- Significantly reduces analysis time for large projects

### Error Codes

DECX uses structured error codes for clear diagnostics:

| Code | Description | HTTP Status |
|------|-------------|-------------|
| **INTERNAL_ERROR** | Internal server error | 500 |
| **SERVICE_ERROR** | Service error | 503 |
| **HEALTH_CHECK_FAILED** | Health check failed | 500 |
| **UNKNOWN_ENDPOINT** | Unknown endpoint | 404 |
| **INVALID_PARAMETER** | Invalid parameter | 400 |
| **METHOD_NOT_FOUND** | Method not found | 404 |
| **CLASS_NOT_FOUND** | Class not found | 404 |
| **RESOURCE_NOT_FOUND** | Resource not found | 404 |
| **MANIFEST_NOT_FOUND** | AndroidManifest not found | 404 |
| **FIELD_NOT_FOUND** | Field not found | 404 |
| **INTERFACE_NOT_FOUND** | Interface not found | 404 |
| **SERVICE_IMPL_NOT_FOUND** | Service implementation not found | 404 |
| **NO_STRINGS_FOUND** | No strings.xml resource found | 404 |
| **NO_MAIN_ACTIVITY** | No MAIN/LAUNCHER Activity found | 404 |
| **NO_APPLICATION** | Application class not found | 404 |
| **EMPTY_SEARCH_KEY** | Search key cannot be empty | 400 |
| **NOT_GUI_MODE** | Not in GUI mode | 503 |

**Error Response Format:**
```json
{
  "ok": false,
  "error": {
    "code": "CLASS_NOT_FOUND",
    "message": "Class not found: com.example.Foo"
  }
}
```

---

## CLI

DECX provides a TypeScript CLI for programmatic access to the analysis platform.

**Installation:**

```bash
npm install -g @jygzyc/decx-cli
```

**Process Management:**
- `decx process check` - Check DECX server status
- `decx process open <file>` - Open and analyze a file (APK, DEX, JAR, etc.); defaults to `--show-bad-code`
- `decx process close [name]` - Stop DECX server by session name
- `decx process list` - List running processes

**Code Analysis:**
- `decx code all-classes` - Get classes (`--first`, `--include-package`, `--exclude-package`)
- `decx code class-context <class>` - Get class information
- `decx code class-source <class>` - Get class source code (`--smali` for Smali output)
- `decx code search-global <keyword> --max-results <n>` - Search globally (`--first`, `--include-package`, `--exclude-package`, `--no-regex`, `--case-sensitive`)
- `decx code search-class <class> <pattern> --max-results <n>` - Grep one class (`--no-regex`, `--case-sensitive`)
- `decx code search-method <name>` - Find methods by name
- `decx code method-source <signature>` - Get method source (`--smali` for Smali output)
- `decx code method-context <signature>` - Get method signature, callers, and callees
- `decx code method-cfg <signature>` - Get method control flow graph as DOT source
- `decx code xref-method <signature>` - Find method callers
- `decx code xref-class <class>` - Find class usages
- `decx code xref-field <field>` - Find field usages
- `decx code implement <interface>` - Find implementations
- `decx code subclass <class>` - Find subclasses

**Android Analysis:**
- `decx ard app-manifest` - Get AndroidManifest.xml
- `decx ard main-activity` - Get main activity name
- `decx ard app-application` - Get Application class name
- `decx ard exported-components [--type <pattern>] [--no-regex]` - List exported components with optional type filtering
- `decx ard app-deeplinks` - List deep link schemes
- `decx ard app-receivers [--first <n>] [--include-package <pattern>] [--exclude-package <pattern>] [--no-regex]` - List dynamic broadcast receivers with package filters
- `decx ard system-service-impl <interface>` - Find system service implementations
- `decx ard system-services [--serial <serial>] [--grep <keyword>]` - List live Android system services as structured JSON
- `decx ard perm-info <permission> [--serial <serial>]` - Show structured Android permission details
- `decx ard all-resources` - List all resource file names
- `decx ard resource-file <res>` - Get resource file content by name
- `decx ard strings` - Get strings.xml content
- `decx ard get-aidl [--first <n>] [--include-package <pattern>] [--exclude-package <pattern>] [--no-regex]` - Get AIDL interfaces with package filters

**Self Management:**
- `decx self install` - Install or update decx-server.jar (`-p` for prerelease)
- `decx self update` - Update decx-server.jar and the currently installed npm CLI package (`-p` affects the server JAR path only)

All session-backed `code` and `ard` commands support `--page <n>` for pagination.
The adb-backed `system-services` and `perm-info` commands use `--serial` / `--adb-path` instead of `-P <port>`.

`system-services` returns structured JSON with `total` and `services[]`, where each service includes `index`, `name`, and `interfaces`.
`perm-info` returns one parsed permission object with fields such as `permission`, `package`, `label`, `description`, and `protectionLevel`.

---

## AI Agent Skills

The `skill/` directory contains AI agent skill definitions that enable automated Android reverse-engineering, vulnerability hunting, and PoC construction.

### Available Skills

| Skill | Description | Dependencies |
|-------|-------------|--------------|
| **decxcli** | General analysis: code navigation, xrefs, manifest/resources inspection | `decx` |
| **decxcli-app-vulnhunt** | App vulnerability hunting: APK attack-surface enumeration, component/WebView/IPC tracing, exploitability triage, bilingual report generation (zh/en) | `decx` |
| **decxcli-framework-vulnhunt** | Framework vulnerability hunting: Binder service enumeration, framework JAR tracing, permission-gate review, exploitability triage, bilingual report generation (zh/en) | `decx` |
| **decxcli-poc** | PoC construction: finding normalization, exploit-class implementation, optional compile/deploy | `decx`, `node` |

Skills are designed to work in sequence: `decxcli` (analysis) → `decxcli-app-vulnhunt` or `decxcli-framework-vulnhunt` (vulnerability hunting) → `decxcli-poc` (PoC construction).

### Prerequisite

Install the DECX CLI before using any skill:

```bash
npm install -g @jygzyc/decx-cli
```

### Installation

__Note:__ Installation differs by platform.

**Claude Code**

```bash
cp -r skill/decxcli ~/.claude/skills/
cp -r skill/decxcli-app-vulnhunt ~/.claude/skills/
cp -r skill/decxcli-framework-vulnhunt ~/.claude/skills/
cp -r skill/decxcli-poc ~/.claude/skills/
```

**Cursor**

```bash
cp skill/decxcli/SKILL.md .cursor/rules/decxcli.md
cp skill/decxcli-app-vulnhunt/SKILL.md .cursor/rules/decxcli-app-vulnhunt.md
cp skill/decxcli-framework-vulnhunt/SKILL.md .cursor/rules/decxcli-framework-vulnhunt.md
cp skill/decxcli-poc/SKILL.md .cursor/rules/decxcli-poc.md
```

**Cline**

```bash
cp skill/decxcli/SKILL.md .clinerules-decxcli
cp skill/decxcli-app-vulnhunt/SKILL.md .clinerules-decxcli-app-vulnhunt
cp skill/decxcli-framework-vulnhunt/SKILL.md .clinerules-decxcli-framework-vulnhunt
cp skill/decxcli-poc/SKILL.md .clinerules-decxcli-poc
```

**Windsurf**

```bash
cp skill/decxcli/SKILL.md .windsurfrules-decxcli
cp skill/decxcli-app-vulnhunt/SKILL.md .windsurfrules-decxcli-app-vulnhunt
cp skill/decxcli-framework-vulnhunt/SKILL.md .windsurfrules-decxcli-framework-vulnhunt
cp skill/decxcli-poc/SKILL.md .windsurfrules-decxcli-poc
```

---

## Development

### Build from Source

```bash
# Build DECX Core
cd decx
chmod +x gradlew
./gradlew dist

# Build CLI
cd cli
npm install
npm run build
```

### Adding Custom Features

Decx's architecture consists of three layers: `DecxApi` interface definition → `DecxApiImpl` implementation → `RouteHandler` HTTP routing.

**1. Add method in `DecxApi` interface**

File: `decx/decx-core/src/main/kotlin/jadx/plugins/decx/api/DecxApi.kt`

```kotlin
interface DecxApi {
    // ... existing methods ...

    fun doSomething(param: String): DecxApiResult
}
```

**2. Implement method in `DecxApiImpl`**

File: `decx/decx-core/src/main/kotlin/jadx/plugins/decx/api/DecxApiImpl.kt`

```kotlin
override fun doSomething(param: String): DecxApiResult {
    val result = // business logic
    return DecxApiResult.ok(mapOf("data" to result))
}
```

**3. Register route in `DecxRoutes`**

File: `decx/decx-core/src/main/kotlin/jadx/plugins/decx/api/DecxApiContract.kt`

```kotlin
DecxRoute("/api/decx/do_something", "do_something") { api, params ->
    api.doSomething(params.requireString("param"))
}
```

### Troubleshooting

**Companion Process Issues:**
- **Check logs**: Look for `[MCP]` messages in DECX logs
- **Verify Python**: Ensure Python 3.10+ or `uv` is installed
- **Check dependencies**: Plugin auto-checks for `requests`, `fastmcp`, `pydantic`
- **Manual path**: Configure custom script path via GUI if needed

**Connection Issues:**
- Use `health_check()` to verify both servers are running
- Check port conflicts: `lsof -i :25419`
- Verify firewall allows localhost connections

**Common Errors:**
- **INTERNAL_ERROR**: Check DECX logs for internal server errors
- **HEALTH_CHECK_FAILED**: Ensure DECX plugin is enabled and loaded
- **INVALID_PARAMETER**: Check parameter format and values

## Contributing

1. Fork this repository
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Submit a Pull Request

---

## License

This project is licensed under [GNU License](LICENSE) - see the [LICENSE](LICENSE) file for details.

---

## Credits

- **[skylot/jadx](https://github.com/skylot/jadx)** - The foundation of this project, a powerful JADX decompiler with plugin support
- **[zinja-coder/jadx-ai-mcp](https://github.com/zinja-coder/jadx-ai-mcp)** - Provided many ideas and inspiration, excellent practices for JADX MCP integration
- **[FastMCP](https://github.com/modelcontextprotocol/servers)**: MCP protocol implementation
- **[Javalin](https://javalin.io/)**: Lightweight web framework

---

<div align="center">

**⭐ If this project helps you, please give it a Star!**

![Star History](https://img.shields.io/github/stars/jygzyc/decx?style=social)

</div>
