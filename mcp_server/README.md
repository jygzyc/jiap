# JIAP MCP Server

MCP server for JIAP (Java Intelligence Analysis Platform), providing AI assistant integration with JADX decompiler through the JIAP plugin.

## Setup

Start the MCP server directly:

```bash
python mcp_server.py
```

The server will start on `http://0.0.0.0:25420` and connect to JIAP plugin on `http://127.0.0.1:25419`.

## Requirements

- Python 3.10+
- JIAP Plugin running in JADX on port 25419
- Dependencies: `requests`, `fastmcp`

## Available Tools

### Basic Analysis

- **`get_all_classes(page)`**: Retrieves all available classes
- **`get_class_source(class_name, smali=False, page=1)`**: Gets class source code in Java/Smali format
- **`get_method_source(method_name, smali=False, page=1)`**: Gets method source code
- **`list_methods(class_name, page=1)`**: Lists all methods in a class
- **`search_class(class_name, page=1)`**: Searches for a class by full name

### Cross-References

- **`get_method_xref(method_name, page=1)`**: Gets method usage locations
- **`get_class_xref(class_name, page=1)`**: Gets class usage locations
- **`get_implement(interface_name, page=1)`**: Gets interface implementations
- **`get_sub_classes(class_name, page=1)`**: Gets subclasses

### Android Analysis

- **`get_app_manifest(page=1)`**: Gets Android manifest content
- **`get_main_activity(page=1)`**: Gets main activity source
- **`get_system_service_impl(interface_name, page=1)`**: Gets system service implementation

### UI Integration

- **`selected_text(page=1)`**: Gets currently selected text in JADX GUI

### Health Check

- **`health_check()`**: Checks if JIAP server is running

## Usage

1. Start JADX with JIAP plugin enabled
2. Verify JIAP server runs on `http://127.0.0.1:25419`
3. Start MCP server: `python mcp_server.py`
4. Use `health_check()` to verify connection

## Notes

- All tools support pagination via `page` parameter
- Server operates on currently loaded JADX project
- Port 25419 is for JIAP plugin (fixed)
- Port 25420 is for MCP server (fixed)