# JIAP MCP Server

Model Context Protocol (MCP) server for JIAP (Java Intelligence Analysis Platform), providing seamless integration between AI assistants and the JADX decompilation engine.

## Quick Start

```json
{
   "mcpServers": {
      "jiap-mcp-server": {
         "command": "uv",
         "args": [
            "--directory",
            "<dir to mcp server>",
            "run",
            "mcp_server.py"
         ]
      }
   }
}
```

## MCP Tools Reference

### File Management Services

#### `process_remote_file`
- **描述**: Downloads and processes a file from a remote URL for decompilation analysis
- **参数**: `url` (string): Remote file URL
- **请求示例**: `{"url": "https://example.com/app.apk"}`
- **响应示例**: `{"success": true, "fileId": "abc123", "message": "File processed successfully"}`

#### `list_files`
- **描述**: Retrieves a list of all uploaded and initialized files
- **参数**: None
- **请求示例**: `{}`
- **响应示例**: `{"files": [{"id": "abc123", "name": "app.apk", "uploadTime": "2024-01-01T12:00:00Z"}]}`

#### `delete_file`
- **描述**: Deletes an uploaded file and removes its decompiler instance
- **参数**: `file_id` (string): File identifier
- **请求示例**: `{"file_id": "abc123"}`
- **响应示例**: `{"success": true, "message": "File deleted successfully"}`

#### `remove_decompiler`
- **描述**: Removes a specific JADX decompiler instance and releases memory
- **参数**: `file_id` (string): File identifier
- **请求示例**: `{"file_id": "abc123"}`
- **响应示例**: `{"success": true, "message": "Decompiler instance removed"}`

#### `remove_all_decompilers`
- **描述**: Removes all JADX decompiler instances and releases all memory
- **参数**: None
- **请求示例**: `{}`
- **响应示例**: `{"success": true, "message": "All decompiler instances removed", "count": 3}`

### Basic Code Analysis Services

#### `get_all_classes`
- **描述**: Retrieves a complete list of all classes in the decompiled project
- **参数**: `file_id` (string): File identifier
- **请求示例**: `{"file_id": "abc123"}`
- **响应示例**: `{"classes": ["com.example.MainActivity", "com.example.utils.Helper", "android.app.Activity"]}`

#### `search_class_by_name`
- **描述**: Searches for classes by name pattern
- **参数**: 
  - `file_id` (string): File identifier
  - `class_name` (string): Class name or pattern
- **请求示例**: `{"file_id": "abc123", "class_name": "MainActivity"}`
- **响应示例**: `{"matches": ["com.example.MainActivity", "com.example.ui.MainActivity"]}`

#### `search_method_by_name`
- **描述**: Searches for methods by name pattern across all classes
- **参数**: 
  - `file_id` (string): File identifier
  - `method_name` (string): Method name or pattern
- **请求示例**: `{"file_id": "abc123", "method_name": "onCreate"}`
- **响应示例**: `{"methods": ["com.example.MainActivity.onCreate(android.os.Bundle):void", "com.example.BaseActivity.onCreate(android.os.Bundle):void"]}`

#### `list_methods_of_class`
- **描述**: Lists all methods in a specific class
- **参数**: 
  - `file_id` (string): File identifier
  - `class_name` (string): Full class name
- **请求示例**: `{"file_id": "abc123", "class_name": "com.example.MainActivity"}`
- **响应示例**: `{"methods": ["onCreate(android.os.Bundle):void", "onResume():void", "handleClick(android.view.View):void"]}`

#### `list_fields_of_class`
- **描述**: Lists all fields in a specific class
- **参数**: 
  - `file_id` (string): File identifier
  - `class_name` (string): Full class name
- **请求示例**: `{"file_id": "abc123", "class_name": "com.example.MainActivity"}`
- **响应示例**: `{"fields": ["TAG: java.lang.String", "button: android.widget.Button", "isInitialized: boolean"]}`

#### `get_class_source`
- **描述**: Retrieves the decompiled source code of a class
- **参数**: 
  - `file_id` (string): File identifier
  - `class_name` (string): Full class name
  - `smali` (boolean, optional): Return Smali code instead of Java (default: false)
- **请求示例**: `{"file_id": "abc123", "class_name": "com.example.MainActivity", "smali": false}`
- **响应示例**: `{"source": "public class MainActivity extends AppCompatActivity {...}"}`

#### `get_method_source`
- **描述**: Retrieves the decompiled source code of a specific method
- **参数**: 
  - `file_id` (string): File identifier
  - `method_info` (string): Method signature in format "className.methodName(paramTypes):returnType"
  - `smali` (boolean, optional): Return Smali code instead of Java (default: false)
- **请求示例**: `{"file_id": "abc123", "method_info": "com.example.MainActivity.onCreate(android.os.Bundle):void", "smali": false}`
- **响应示例**: `{"source": "@Override\nprotected void onCreate(Bundle savedInstanceState) {...}"}`

#### `get_app_manifest`
- **描述**: Retrieves the AndroidManifest.xml content
- **参数**: `file_id` (string): File identifier
- **请求示例**: `{"file_id": "abc123"}`
- **响应示例**: `{"manifest": "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<manifest xmlns:android=\"http://schemas.android.com/apk/res/android\" package=\"com.example\">..."}`

### Advanced Code Analysis Services

#### `get_method_xref`
- **描述**: Finds all cross-references (usages) of a method
- **参数**: 
  - `file_id` (string): File identifier
  - `method_info` (string): Method signature
- **请求示例**: `{"file_id": "abc123", "method_info": "com.example.Utils.helper(java.lang.String):boolean"}`
- **响应示例**: `{"references": [{"class": "com.example.MainActivity", "method": "onCreate", "line": 25, "type": "call"}, {...}]}`

#### `get_class_xref`
- **描述**: Finds all cross-references (usages) of a class
- **参数**: 
  - `file_id` (string): File identifier
  - `class_name` (string): Full class name
- **请求示例**: `{"file_id": "abc123", "class_name": "com.example.Utils"}`
- **响应示例**: `{"references": [{"class": "com.example.MainActivity", "line": 15, "type": "import"}, {...}]}`

#### `get_field_xref`
- **描述**: Finds all cross-references (usages) of a field
- **参数**: 
  - `file_id` (string): File identifier
  - `field_info` (string): Field signature in format "className.fieldName: type"
- **请求示例**: `{"file_id": "abc123", "field_info": "com.example.Config.API_URL: java.lang.String"}`
- **响应示例**: `{"references": [{"class": "com.example.NetworkManager", "method": "makeRequest", "line": 18, "type": "read"}]}`

#### `get_interface_impl`
- **描述**: Finds all classes that implement a specific interface
- **参数**: 
  - `file_id` (string): File identifier
  - `interface_name` (string): Full interface name
- **请求示例**: `{"file_id": "abc123", "interface_name": "java.lang.Runnable"}`
- **响应示例**: `{"implementations": ["com.example.BackgroundTask", "com.example.NetworkWorker"]}`

#### `get_implement_of_interface`
- **描述**: Finds all interfaces implemented by a specific class
- **参数**: 
  - `file_id` (string): File identifier
  - `class_name` (string): Full class name
- **请求示例**: `{"file_id": "abc123", "class_name": "com.example.MainActivity"}`
- **响应示例**: `{"interfaces": ["android.view.View.OnClickListener", "com.example.CustomListener"]}`

#### `get_subclasses`
- **描述**: Finds all subclasses of a specific class
- **参数**: 
  - `file_id` (string): File identifier
  - `class_name` (string): Full class name
- **请求示例**: `{"file_id": "abc123", "class_name": "com.example.BaseActivity"}`
- **响应示例**: `{"subclasses": ["com.example.MainActivity", "com.example.SettingsActivity"]}`

#### `get_system_service_impl`
- **描述**: Analyzes Android system service implementations and usage
- **参数**: 
  - `file_id` (string): File identifier
  - `service_name` (string): System service name
- **请求示例**: `{"file_id": "abc123", "service_name": "LocationManager"}`
- **响应示例**: `{"usages": [{"class": "com.example.LocationService", "method": "getCurrentLocation", "line": 45, "operation": "getLastKnownLocation"}]}`

### Testing Services

#### `test_endpoint`
- **描述**: Tests the JADX decompiler service connectivity and validates server status
- **参数**: None
- **请求示例**: `{}`
- **响应示例**: `{"status": "ok", "server": "JADX Server v1.4.7", "timestamp": "2024-01-01T12:00:00Z"}`

## Configuration

### Environment Variables
- `JADX_SERVER_URL`: JADX server URL (default: http://127.0.0.1:8080)
- `LOG_LEVEL`: Logging level (default: ERROR)

### Requirements
- Python 3.10+
- JIAP Server running
- Dependencies: `httpx`, `fastmcp`

## License

Apache License - Part of JIAP (Java Intelligence Analysis Platform)