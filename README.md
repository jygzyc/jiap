# JIAP - Java Intelligence Analysis Platform

A comprehensive platform for Java bytecode analysis and Android application reverse engineering, integrating JADX decompiler with AI assistant capabilities through MCP (Model Context Protocol).

## Installation

### Prerequisites
- Java 17+ for JIAP Core
- Python 3.10+ for MCP Server
- JADX decompiler with plugin support

### Build from Source

```bash
# Build JIAP Core
cd jiap_core
chmod +x gradlew
./gradlew build

# Build MCP Server
cd mcp_server
uv sync --all-extras
uv build
```

## Usage

### 1. Start JIAP Plugin
- Launch JADX with JIAP plugin enabled
- Verify the server runs on `http://127.0.0.1:25419`

### 2. Configure MCP Client
Add to your MCP client configuration:

```json
{
   "mcpServers": {
      "jiap-mcp-server": {
         "command": "uv",
         "args": [
            "--directory",
            "/path/to/mcp_server",
            "run",
            "mcp_server.py"
         ]
      }
   }
}
```

### 3. Available Tools

#### Basic Analysis
- `get_all_classes(page)` - Retrieve all available classes
- `get_class_source(class_name, smali=False, page=1)` - Get class source code
- `get_method_source(method_name, smali=False, page=1)` - Get method source code
- `list_methods(class_name, page=1)` - List methods in a class
- `search_class(class_name, page=1)` - Search for classes

#### Cross-References
- `get_method_xref(method_name, page=1)` - Find method usage locations
- `get_class_xref(class_name, page=1)` - Find class usage locations
- `get_implement(interface_name, page=1)` - Get interface implementations
- `get_sub_classes(class_name, page=1)` - Get subclasses

#### Android Analysis
- `get_app_manifest(page=1)` - Get Android manifest content
- `get_main_activity(page=1)` - Get main activity source
- `get_system_service_impl(interface_name, page=1)` - Get system service implementations

#### UI Integration
- `selected_text(page=1)` - Get currently selected text in JADX GUI
- `health_check()` - Check server status

## Development

### Building
```bash
# Build both components
./gradlew build          # JIAP Core
cd mcp_server && uv build # MCP Server
```

### Testing
```bash
# Run tests
./gradlew test           # JIAP Core tests
cd mcp_server && uv run pytest # MCP Server tests
```

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Submit a pull request

## License

This project is licensed under the Apache License 2.0 - see the LICENSE file for details.

## Repository

https://github.com/jygzyc/jiap