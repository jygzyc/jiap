# JIAP - Java Intelligence Analysis Platform

A comprehensive platform for Java bytecode analysis and Android application reverse engineering, integrating JADX decompiler with AI assistant capabilities through MCP (Model Context Protocol).

## Installation

### Prerequisites
- Java 17+ for JIAP Core
- Python 3.10+ for MCP Server
- JADX decompiler with plugin support
- Python dependencies: `requests`, `fastmcp`

### Build from Source

```bash
# Build JIAP Core
cd jiap_core
chmod +x gradlew
./gradlew dist

# Install MCP Server dependencies
cd mcp_server
pip install requests fastmcp
```

## Usage

### 1. Start JIAP Plugin

```bash
# JADX Command Line 
jadx plugins --install-jar <path-to-jiap.jar>

# JADX GUI 
# JADX -> Plugins -> Install from JAR
```

- Launch JADX with JIAP plugin enabled
- Verify the server runs on `http://127.0.0.1:25419`

### 2. Start MCP Server
```bash
cd mcp_server
python jiap_mcp_server.py
```

The MCP server will start on `http://0.0.0.0:25420`.

### 3. Verify Connection
Use `health_check()` to verify the connection between MCP server and JIAP plugin.

### 4. Available Tools

- `get_all_classes(page=1)` - Retrieve all available classes with pagination
- `get_class_source(class_name, smali=False, page=1)` - Get class source code in Java or Smali format
- `get_method_source(method_name, smali=False, page=1)` - Get method source code
- `get_class_info(class_name, page=1)` - Get class information including fields and methods
- `get_method_xref(method_name, page=1)` - Find method usage locations
- `get_class_xref(class_name, page=1)` - Find class usage locations
- `get_implement(interface_name, page=1)` - Get interface implementations
- `get_sub_classes(class_name, page=1)` - Get subclasses
- `get_app_manifest(page=1)` - Get Android manifest content
- `get_main_activity(page=1)` - Get main activity source
- `get_system_service_impl(interface_name, page=1)` - Get system service implementations
- `selected_text(page=1)` - Get currently selected text in JADX GUI
- `health_check()` - Check server status

## Development

### Building
```bash
# Build JIAP Core
./gradlew dist

# MCP Server is ready to run (no build required)
```

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Submit a pull request

## License

This project is licensed under the GNU License 3.0 - see the LICENSE file for details.

## Repository

https://github.com/jygzyc/jiap