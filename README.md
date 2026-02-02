# JIAP - Java Intelligence Analysis Platform

A comprehensive platform for Java bytecode analysis and Android application reverse engineering, integrating JADX decompiler with AI assistant capabilities through MCP (Model Context Protocol).

## Installation

### Prerequisites
- Java 17+ for JIAP Core
- Python 3.10+ for MCP Server (optional - auto-managed)
- JADX decompiler with plugin support (v1.5.2+)
- Python dependencies: `requests`, `fastmcp`, `pydantic`

### Build from Source

```bash
# Build JIAP Core
cd jiap_core
chmod +x gradlew
./gradlew dist

# The MCP server is bundled and auto-managed by the plugin
# No manual Python dependency installation required
```

## Usage

### 1. Install and Start JIAP Plugin

```bash
# JADX Command Line 
jadx plugins --install-jar <path-to-jiap.jar>

# JADX GUI 
# JADX -> Settings -> Plugins -> Enable JIAP
```

**Companion Process Mechanism:**
- When JIAP plugin starts, it automatically launches the MCP server as a **companion process**
- The MCP server runs as a sidecar process managed by the plugin
- No manual MCP server startup required
- The companion process automatically stops when JIAP plugin unloads or JADX exits

**What happens automatically:**
1. JIAP plugin starts HTTP server on port `25419`
2. Plugin extracts MCP scripts to `~/.jiap/mcp/`
3. Companion process (MCP server) starts on port `25420`
4. Plugin monitors companion process health
5. Both processes stop together on shutdown

### 2. Verify Connection
Use `health_check()` to verify the connection between MCP server and JIAP plugin.

### 3. Available Tools

All tools support pagination via the `page` parameter.

**Code Analysis**
- `get_all_classes(page=1)` - Retrieve all available classes
- `search_class_key(key, page=1)` - Search for classes by keyword in source code (case-insensitive)
- `get_class_source(class_name, smali=False, page=1)` - Get class source code in Java or Smali format
- `search_method(method_name, page=1)` - Search for methods matching the name
- `get_method_source(method_name, smali=False, page=1)` - Get method source code using full signature
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
- `get_system_service_impl(interface_name, page=1)` - Get system service implementations

**Vulnerability Mining**
- `get_dynamic_receivers(page=1)` - Get dynamically registered BroadcastReceivers

**System**
- `health_check()` - Verify server connection status

## Configuration

### Port Configuration
The JIAP server port can be configured via:
- **GUI**: JIAP Server Status menu → Set new port → Auto-restart
- **Plugin Options**: Set `jiap.port` in JADX plugin options
- **Default**: `25419` (JIAP), `25420` (MCP companion)

### MCP Script Path
If you want to use a custom MCP server script:
- **GUI**: JIAP Server Status menu → Browse and select custom script
- **Plugin Options**: Set `jiap.mcp_path` to custom script path
- **Default**: Auto-extracted to `~/.jiap/mcp/jiap_mcp_server.py`

### Companion Process Configuration
The companion process (MCP server) is automatically configured:
```bash
# Auto-detected executor: uv, python3, or python
# Auto-extracted scripts to ~/.jiap/mcp/
# Auto-started with correct JIAP_URL and MCP_PORT
# Auto-monitored and restarted if needed
```

## Error Codes

JIAP uses structured error codes for clear diagnostics:

| Code | Description | Common Cause |
|------|-------------|--------------|
| **E001** | Internal server error | Unexpected server state |
| **E002** | Port in use | Another service using the port |
| **E003** | Server start failed | Port binding or initialization error |
| **E004** | Server stop failed | Graceful shutdown timeout |
| **E005** | Server restart failed | Restart sequence error |
| **E006** | JADX unavailable | Decompiler not initialized |
| **E007** | JADX init failed | Decompiler initialization error |
| **E008** | Python not found | No Python/uv executable found |
| **E009** | Sidecar script not found | MCP script extraction failed |
| **E010** | Sidecar start failed | Companion process failed to start |
| **E011** | Sidecar process error | Companion process crashed |
| **E012** | Sidecar stop failed | Companion process won't stop |
| **E013** | Service error | General service failure |
| **E014** | Health check failed | Cannot reach JIAP server |
| **E015** | Method not found | Requested method doesn't exist |
| **E016** | Missing parameter | Required parameter not provided |
| **E017** | Invalid parameter | Parameter format/value invalid |
| **E018** | Unknown endpoint | Requested API endpoint doesn't exist |
| **E019** | Connection error | Network/HTTP connection failed |

**Error Response Format:**
```json
{
  "error": "E010",
  "message": "Sidecar start failed: Python executable not found"
}
```

## Architecture

### Component Overview
```
┌─────────────────────────────────────────────────────────┐
│                    JADX GUI                              │
│  ┌─────────────────────────────────────────────────┐   │
│  │ JIAP Plugin (Kotlin)                            │   │
│  │  - HTTP Server (Port 25419)                     │   │
│  │  - Sidecar Manager                              │   │
│  │  - UI Integration                               │   │
│  └─────────────────────────────────────────────────┘   │
│              │  Process Management                      │
│              ▼                                          │
│  ┌─────────────────────────────────────────────────┐   │
│  │ MCP Companion Process (Python)                  │   │
│  │  - FastMCP Server (Port 25420)                  │   │
│  │  - HTTP Client to JIAP                          │   │
│  │  - Tool Definitions                             │   │
│  └─────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
              │
              ▼
       AI Assistant (Claude, etc.)
```

### Companion Process Lifecycle
1. **Plugin Init**: JIAP plugin loads and initializes
2. **Server Start**: HTTP server starts on configured port
3. **Script Extraction**: MCP scripts extracted to `~/.jiap/mcp/`
4. **Companion Launch**: Background thread starts companion process
5. **Health Monitoring**: Process monitored for crashes
6. **Auto-Restart**: Failed processes automatically restarted
7. **Clean Shutdown**: Both processes stop together on unload

## Troubleshooting

### Companion Process Issues
- **Check logs**: Look for `[MCP Sidecar STDOUT]` messages in JIAP logs
- **Verify Python**: Ensure Python 3.10+ or `uv` is installed
- **Check dependencies**: Plugin auto-checks for `requests`, `fastmcp`, `pydantic`
- **Manual path**: Configure custom script path via GUI if needed

### Connection Issues
- Use `health_check()` to verify both servers are running
- Check port conflicts with `netstat -tlnp | grep 25419`
- Verify firewall allows localhost connections

### Common Errors
- **E008**: Install Python 3.10+ or `uv`
- **E009/E010**: Check `~/.jiap/mcp/` permissions
- **E002**: Change port in JIAP settings GUI
- **E014**: Ensure JIAP plugin is enabled and loaded

## Development

```bash
# Build JIAP Core
cd jiap_core
./gradlew dist

# Test MCP Server (optional, for development)
cd mcp_server
python jiap_mcp_server.py --url "http://127.0.0.1:25419"
```

## License

GNU License 3.0 - see the LICENSE file for details.

## Credits

- [skylot/jadx](https://github.com/skylot/jadx) - The foundation of this project, a powerful JADX decompiler with plugin support
- [zinja-coder/jadx-ai-mcp](https://github.com/zinja-coder/jadx-ai-mcp) - Inspired many ideas and approaches for JADX MCP integration