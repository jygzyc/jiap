# JIAP - Java Intelligence Analysis Platform

<div align="center">

![JIAP Logo](https://img.shields.io/badge/JIAP-Java%20Intelligence%20Analysis%20Platform-blue?style=for-the-badge&logo=java&logoColor=white)
![Release](https://img.shields.io/github/v/release/jygzyc/jiap?style=for-the-badge&logo=github&color=green)
![License](https://img.shields.io/github/license/jygzyc/jiap?style=for-the-badge&logo=gnu&color=orange)

**A JADX-based Java Intelligence Analysis Platform - Designed for AI-assisted code analysis**

</div>

---

## Overview

JIAP (Java Intelligence Analysis Platform) is a smart code analysis platform built on the JADX decompiler, designed specifically for AI-assisted code analysis. The platform provides powerful Java code analysis capabilities to AI assistants through HTTP API and MCP (Model Context Protocol).

---

## Quick Start

### Prerequisites

- **Java**: JDK 11+ (for JIAP Core)
- **Python**: 3.10+ (optional - auto-managed)
- **JADX**: v1.5.2+ with plugin support
- **Python dependencies**: `requests`, `fastmcp`, `pydantic` (auto-installed)

### Installation

```bash
# 1. Install plugin in JADX GUI
# JADX -> Settings -> Plugins -> Install JIAP

# Or install via command line
jadx plugins --install-jar <path-to-jiap.jar>

# 2. Build JIAP Core (from source)
cd jiap
chmod +x gradlew
./gradlew dist
```

**MCP Server Auto-Management:**
- Plugin automatically extracts and manages MCP server scripts
- No manual Python dependency installation or MCP server startup required
- No manual environment variable configuration required

### Usage

* Start JIAP Plugin

  - Launch JADX and enable the JIAP plugin
  - The plugin automatically starts the HTTP server, MCP server can be manually confirmed to start
  - Verify the server is running at `http://127.0.0.1:25419`

**Automatic Process:**
1. JIAP plugin starts HTTP server (port `25419`)
2. Plugin extracts MCP scripts to `~/.jiap/mcp/`
3. If auto-start is enabled, companion process (MCP server) starts automatically (port `25419 + 1`)
4. Both processes stop together on shutdown

* Verify Connection

Use `health_check()` to verify the connection between MCP server and JIAP plugin

* Available Tools

All tools support pagination via the `page` parameter.

**Code Analysis**
- `get_all_classes(page=1)` - Retrieve all available classes
- `search_class_key(key, page=1)` - Search for classes by keyword in source code (case-insensitive)
- `get_class_source(class_name, smali=False, page=1)` - Get class source code in Java or Smali format
- `search_method(method_name, page=1)` - Search for methods matching the name
- `get_method_source(method_name, smali=False, page=1)` - Get method source code
- `get_class_info(class_name, page=1)` - Get class information (fields and methods)
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
- `get_exported_components(page=1)` - Get exported components (activities, services, receivers, providers) with permissions
- `get_deep_links(page=1)` - Get app URL schemes and intent filters
- `get_all_resources(page=1)` - List all resource file names (including resources.arsc sub-files)
- `get_resource_file(resource_name, page=1)` - Get resource file content by name
- `get_strings(page=1)` - Get strings.xml content from app resources
- `get_dynamic_receivers(page=1)` - Get dynamically registered BroadcastReceivers
- `get_aidl(page=1)` - Get all AIDL interfaces and their implementations
- `get_system_service_impl(interface_name, page=1)` - Get system service implementations

**System**
- `health_check()` - Verify server connection status

### Configuration

**Port Configuration:**
- **GUI**: JIAP Server Status menu → Set new port → Auto-restart
- **Plugin Options**: Set `jiap.port` in JADX plugin options
- **Default**: `25419` (JIAP)

**MCP Script Path:**
- **GUI**: JIAP Server Status menu → Browse and select custom script
- **Plugin Options**: Set `jiap.mcp_path` to custom script path
- **Default**: Auto-extracted to `~/.jiap/mcp/jiap_mcp_server.py`

**Companion Process Configuration:**
```bash
# Auto-detected executor: uv, python3, or python
# Auto-extracted scripts to ~/.jiap/mcp/
# Auto-started with correct JIAP_URL and MCP_PORT
```

**Cache Configuration:**
JIAP supports two cache modes for improved performance:
- **disk** (default): Persists decompilation cache to disk (`~/.jiap/cache/`)
- **memory**: Keeps cache in memory only, suitable for small projects

**Configuration:**
- **Plugin Options**: Set `jiap.cache` to `disk` or `memory`
- **Default**: `disk` for better performance on subsequent runs

**Performance Optimization:**
JIAP includes automatic performance optimizations:

**Decompiler Warmup:**
- JIAP automatically warms up the decompiler engine on startup
- Filters out SDK packages (android.*, androidx.*, java.*, javax.*, kotlin.*)
- Randomly samples up to 15,000 application classes
- Ensures optimal performance for subsequent queries

**Disk Caching:**
- Decompiled code is cached to disk for faster retrieval
- Cache persists across JADX sessions
- Significantly reduces analysis time for large projects

### Error Codes

JIAP uses structured error codes for clear diagnostics:

| Code | Description | Common Cause |
|------|-------------|--------------|
| **E001** | Internal server error | Unexpected server state |
| **E002** | Service error | General service failure |
| **E003** | Health check failed | Cannot reach JIAP server |
| **E004** | Method not found | Requested method doesn't exist |
| **E005** | Invalid parameter | Parameter format/value invalid |

**Error Response Format:**
```json
{
  "error": "E001",
  "message": "Internal error: Start failed"
}
```

---

## CLI

JIAP provides a TypeScript CLI for programmatic access to the analysis platform.

**Installation:**

```bash
npm install -g jiap-cli
```

**Process Management:**
- `jiap process check` - Check JIAP server status
- `jiap process open <file>` - Open and analyze a file (APK, DEX, JAR, etc.)
- `jiap process close [name]` - Stop JIAP server by session name
- `jiap process list` - List running processes
- `jiap process install` - Install or update jiap-server.jar

**Code Analysis:**
- `jiap code all-classes` - Get all classes
- `jiap code class-info <class>` - Get class information
- `jiap code class-source <class>` - Get class source code
- `jiap code search-class <keyword>` - Search in class content
- `jiap code search-method <name>` - Find methods by name
- `jiap code method-source <signature>` - Get method source
- `jiap code xref-method <signature>` - Find method callers
- `jiap code xref-class <class>` - Find class usages
- `jiap code xref-field <field>` - Find field usages
- `jiap code implement <interface>` - Find implementations
- `jiap code subclass <class>` - Find subclasses
- `jiap code get-aidl` - Get all AIDL interfaces

**Android Analysis:**
- `jiap ard app-manifest` - Get AndroidManifest.xml
- `jiap ard main-activity` - Get main activity name
- `jiap ard app-application` - Get Application class name
- `jiap ard exported-components` - List exported components
- `jiap ard app-deeplinks` - List deep link schemes
- `jiap ard app-receivers` - List dynamic broadcast receivers
- `jiap ard system-service-impl <interface>` - Find system service implementations
- `jiap ard all-resources` - List all resource file names
- `jiap ard resource-file <res>` - Get resource file content by name
- `jiap ard strings` - Get strings.xml content

---

## AI Agent Skill Installation

The `skill/` directory contains JIAP's AI Agent skill definition files (SKILL.md), supporting the following AI assistants:

**Claude Code**
```bash
cp -r skill/jiapcli ~/.claude/skills/
cp -r skill/jiapcli-vulnhunt ~/.claude/skills/
cp -r skill/jiapcli-poc ~/.claude/skills/
```

**Cursor**
```bash
cp skill/jiapcli/SKILL.md .cursor/rules/jiapcli.md
cp skill/jiapcli-vulnhunt/SKILL.md .cursor/rules/jiapcli-vulnhunt.md
cp skill/jiapcli-poc/SKILL.md .cursor/rules/jiapcli-poc.md
```

**Cline**
```bash
cp skill/jiapcli/SKILL.md .clinerules-jiapcli
cp skill/jiapcli-vulnhunt/SKILL.md .clinerules-jiapcli-vulnhunt
cp skill/jiapcli-poc/SKILL.md .clinerules-jiapcli-poc
```

**Windsurf**
```bash
cp skill/jiapcli/SKILL.md .windsurfrules-jiapcli
cp skill/jiapcli-vulnhunt/SKILL.md .windsurfrules-jiapcli-vulnhunt
cp skill/jiapcli-poc/SKILL.md .windsurfrules-jiapcli-poc
```

Dependency: `jiap` CLI installed (`npm install -g jiap-cli`).

---

## Development

### Build from Source

```bash
# Build JIAP Core
cd jiap
chmod +x gradlew
./gradlew dist

# Build CLI
cd cli
npm install
npm run build
```

### Adding Custom Features

JIAP's architecture consists of three layers: `JiapApi` interface definition → `JiapApiImpl` implementation → `RouteHandler` HTTP routing.

**1. Add method in `JiapApi` interface**

File: `jiap/jiap-core/src/main/kotlin/jadx/plugins/jiap/api/JiapApi.kt`

```kotlin
interface JiapApi {
    // ... existing methods ...

    fun doSomething(param: String): JiapApiResult
}
```

**2. Implement method in `JiapApiImpl`**

File: `jiap/jiap-core/src/main/kotlin/jadx/plugins/jiap/api/JiapApiImpl.kt`

```kotlin
override fun doSomething(param: String): JiapApiResult {
    val result = // business logic
    return JiapApiResult.ok(mapOf("data" to result))
}
```

**3. Register route in `JiapServer.ALL_ROUTES`, add dispatch in `RouteHandler.dispatch()`**

File: `jiap/jiap-core/src/main/kotlin/jadx/plugins/jiap/http/JiapServer.kt`

```kotlin
val ALL_ROUTES = setOf(
    // ... existing routes ...
    "/api/jiap/do_something",
)
```

File: `jiap/jiap-core/src/main/kotlin/jadx/plugins/jiap/http/RouteHandler.kt`

```kotlin
"/api/jiap/do_something" -> requireParam(payload, "param") { api.doSomething(it) }
```

### Troubleshooting

**Companion Process Issues:**
- **Check logs**: Look for `[MCP]` messages in JIAP logs
- **Verify Python**: Ensure Python 3.10+ or `uv` is installed
- **Check dependencies**: Plugin auto-checks for `requests`, `fastmcp`, `pydantic`
- **Manual path**: Configure custom script path via GUI if needed

**Connection Issues:**
- Use `health_check()` to verify both servers are running
- Check port conflicts: `lsof -i :25419`
- Verify firewall allows localhost connections

**Common Errors:**
- **E001**: Check JIAP logs for internal server errors
- **E003**: Ensure JIAP plugin is enabled and loaded
- **E005**: Check parameter format and values

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

![Star History](https://img.shields.io/github/stars/jygzyc/jiap?style=social)

</div>
