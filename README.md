# JIAP - Java Intelligence Analysis Platform

An AI-powered Java application analysis platform built on JADX decompiler, providing comprehensive code analysis capabilities through Model Context Protocol (MCP) for AI assistants.

## Overview

JIAP is a modular platform that combines the power of JADX decompilation with AI-driven analysis capabilities. It consists of three core modules: file preprocessing, JADX server, and MCP server, designed to provide seamless Java code analysis for AI assistants and developers.

**Warning**: Please note that decompilation may not be 100% accurate in all cases, and errors may occur during the analysis process.

## Main Features

- **Comprehensive Decompilation**: Support for APK, DEX, JAR, APEX, and other Android file formats
- **AI Integration**: Native MCP support for seamless AI assistant integration
- **Advanced Analysis**: Cross-reference analysis, inheritance relationships, and system service mapping
- **RESTful API**: Complete HTTP API for programmatic access
- **File Preprocessing**: Automated extraction and processing of Android framework files
- **Multi-format Support**: Java and Smali source code output

## Architecture

```txt
├── processor_tools/
│   └── file_preprocessor/     # File preprocessing module
├── jadx_server/              # JADX decompilation server
└── mcp_server/              # MCP protocol server
```

## Requirements

- Java 11 or later (64-bit version recommended)
- Python 3.8+
- Android SDK (optional, for device file extraction)

## Installation

### Build from Source

```bash
git clone https://github.com/jygzyc/JIAP.git
cd JIAP
```

### 1. Start JIAP Server

```bash
cd jiap_server
./gradlew dist
# Run with the provided script
./run_jiap_server.sh
```

### 2. Start MCP Server

```bash
cd mcp_server
# Install dependencies
pip install -r requirements.txt
# Or using uv
uv sync
```

#### MCP Configuration

Add the following configuration to your AI assistant's MCP settings:

```json
{
   "mcpServers": {
      "jiap-mcp-server": {
         "command": "uv",
         "args": [
            "--directory",
            "<path_to_mcp_server_directory>",
            "run",
            "mcp_server.py"
         ]
      }
   }
}
```

#### Environment Variables
- `JADX_SERVER_URL`: JADX server URL (default: http://127.0.0.1:8080)
- `LOG_LEVEL`: Logging level (default: ERROR)

### 3. File Preprocessing (Optional)

```bash
cd processor_tools/file_preprocessor
python fetch_file_from_device.py  # Extract files from device
python extract_framework_code.py  # Process framework files
```

## Usage

### Available Restful APIs

**File Management Services:**

- `local_handle` - Upload local file
- `remote_handle` - Download and process files from remote URLs
- `list` - List all uploaded files
- `delete` - Delete uploaded files

**Jadx Common Services:**

- `remove_decompiler` - Remove specific decompiler instances
- `remove_all_decompilers` - Remove all decompiler instances
- `get_all_classes` - Get complete list of classes
- `search_class_by_name` - Search classes by class full name
- `search_method_by_name` - Search methods by method short name
- `list_methods_of_class` - List all methods in a class
- `list_fields_of_class` - List all fields in a class
- `get_class_source` - Get decompiled source code of a class whether is smali or not
- `get_method_source` - Get decompiled source code of a method whether is smali or not
- `get_method_xref` - Find method cross-references
- `get_class_xref` - Find class cross-references
- `get_field_xref` - Find field cross-references
- `get_interface_impl` - Find the interface implements
- `get_subclasses` - Find subclasses of a class

**Advanced Code Analysis Services:**

- `get_app_manifest` - Get AndroidManifest.xml content
- `get_system_service_impl` - Get Android System service implement of the interface

**Testing Services:**

- `test` - Test server connectivity

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- [JADX](https://github.com/skylot/jadx) - The core decompilation engine
- [Spring Boot](https://spring.io/projects/spring-boot) - Web framework for the server component
- [Model Context Protocol](https://modelcontextprotocol.io/) - AI assistant integration protocol
