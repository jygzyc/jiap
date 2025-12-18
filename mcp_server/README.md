# JIAP MCP Server

MCP server for JIAP (Java Intelligence Analysis Platform), providing AI assistant integration with JADX decompiler through the JIAP plugin.

## Setup

Start the MCP server directly:

```bash
python jiap_mcp_server.py
# uv run jiap_mcp_server.py
```

The server will start on `http://0.0.0.0:25420` and connect to JIAP plugin on `http://127.0.0.1:25419`.

### Configuration Options

The JIAP server URL can be configured using:

**1. Environment Variable:**
```bash
export JIAP_URL="http://192.168.1.100:25419"
python jiap_mcp_server.py
```

**2. Command Line Arguments:**
```bash
python jiap_mcp_server.py --jiap-host 192.168.1.100 --jiap-port 25419
```

**3. Full URL Override:**
```bash
python jiap_mcp_server.py --jiap-url "http://192.168.1.100:25419"
```

**Priority:** Full URL > Command line arguments > Environment variables > Default values

**Available Environment Variables:**
- `JIAP_URL` - Full JIAP server URL
- `JIAP_HOST` - JIAP server host (default: 127.0.0.1)
- `JIAP_PORT` - JIAP server port (default: 25419)

## Requirements

- Python 3.10+
- JIAP Plugin running in JADX on port 25419
- Dependencies: `requests`, `fastmcp`

## Available Tools

### Basic JADX Analysis
- `get_all_classes(page=1)` - Retrieve all available classes with pagination
- `search_class_key(key, page=1)` - Search for classes whose source code contains the specified keyword (case-insensitive)
- `get_class_source(class_name, smali=False, page=1)` - Get class source code in Java or Smali format (e.g., `com.example.MyClass$InnerClass`)
- `get_class_info(class_name, page=1)` - Get class information including fields and methods
- `get_class_xref(class_name, page=1)` - Find class usage locations

- `search_method(method_name, page=1)` - Search for methods by name (e.g., `doSomething` matches `com.example.Service.doSomething`)
- `get_method_source(method_name, smali=False, page=1)` - Get method source code using full signature (e.g., `com.example.MyClass.method(String):int`)
- `get_method_xref(method_name, page=1)` - Find method usage locations

- `get_implement(interface_name, page=1)` - Get interface implementations
- `get_sub_classes(class_name, page=1)` - Get subclasses

### UI Integration
- `selected_text(page=1)` - Get currently selected text in JADX GUI
- `selected_class(page=1)` - Get currently selected class in JADX GUI

### Android App Analysis
- `get_app_manifest(page=1)` - Get Android manifest content
- `get_main_activity(page=1)` - Get main activity source
- `get_application(page=1)` - Get Android application class and its information

### Android Framework
- `get_system_service_impl(interface_name, page=1)` - Get system service implementations (e.g., `android.os.IMyService`)

### System
- `health_check()` - Check server status

## Usage

1. Start JADX with JIAP plugin enabled
2. Verify JIAP server runs on `http://127.0.0.1:25419`
3. Start MCP server: `python jiap_mcp_server.py`
4. Use `health_check()` to verify connection

## Notes

- All tools support pagination via `page` parameter
- Server operates on currently loaded JADX project
- Port 25419 is for JIAP plugin (default)
- Port 25420 is for MCP server (fixed)