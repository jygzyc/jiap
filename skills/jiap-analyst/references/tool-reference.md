# JIAP Tool Reference

MCP tools for Android security analysis via JIAP plugin.

## System

| Tool | Description | Parameters |
|------|-------------|------------|
| `health_check()` | Check server status | None |

## Code Retrieval

| Tool | Description | Parameters |
|------|-------------|------------|
| `get_all_classes(page)` | List all decompiled class names | `page`: Page number for pagination (default: 1) |
| `get_class_source(class_name, smali, page)` | Get Java/Smali source for a class | `class_name`: Full class name (use `$` for inner classes)<br>`smali`: Return Smali format (default: false)<br>`page`: Page number |
| `get_method_source(method_name, smali, page)` | Get source for specific method | `method_name`: Full signature<br>`smali`: Return Smali format (default: false)<br>`page`: Page number |
| `get_class_info(class_name, page)` | Get class metadata (fields, methods) | `class_name`: Full class name<br>`page`: Page number |
| `get_app_manifest(page)` | Get AndroidManifest.xml content | `page`: Page number |

## Search

| Tool | Description | Parameters |
|------|-------------|------------|
| `search_method(method_name, page)` | Find methods by name pattern | `method_name`: Method name or partial name<br>`page`: Page number |
| `search_class_key(keyword, page)` | Search class source for keyword | `keyword`: Search term<br>`page`: Page number |

## Cross-Reference

| Tool | Description | Parameters |
|------|-------------|------------|
| `get_method_xref(method_name, page)` | Find all callers of a method | `method_name`: Full method signature<br>`page`: Page number |
| `get_field_xref(field_name, page)` | Find all references to a field | `field_name`: Full field signature<br>`page`: Page number |
| `get_class_xref(class_name, page)` | Find all usages of a class | `class_name`: Full class name<br>`page`: Page number |

## Structure Analysis

| Tool | Description | Parameters |
|------|-------------|------------|
| `get_sub_classes(class_name, page)` | Find all subclasses | `class_name`: Parent class name<br>`page`: Page number |
| `get_implement(interface_name, page)` | Find interface implementations | `interface_name`: Interface name<br>`page`: Page number |

## Android-Specific

| Tool | Description | Parameters |
|------|-------------|------------|
| `get_exported_components(page)` | List exported activities/services/receivers/providers | `page`: Page number |
| `get_deep_links(page)` | Get deep link URL schemes | `page`: Page number |
| `get_dynamic_receivers(page)` | Find dynamically registered receivers | `page`: Page number |
| `get_system_service_impl(interface_name, page)` | Map AIDL interface to implementation | `interface_name`: Interface name (e.g., `android.os.IPowerManager`)<br>`page`: Page number |
| `get_main_activity(page)` | Get main launcher activity | `page`: Page number |
| `get_application(page)` | Get Application class | `page`: Page number |

## UI Integration

| Tool | Description | Parameters |
|------|-------------|------------|
| `selected_text(page)` | Get text selected in JADX GUI | `page`: Page number |
| `selected_class(page)` | Get class selected in JADX GUI | `page`: Page number |

---

## Signature Formats

**Method:** `package.Class.methodName(paramType1, paramType2):returnType`

**Examples:**
- `com.example.MainActivity.onCreate(android.os.Bundle):void`
- `com.example.Crypto.decrypt(java.lang.String):java.lang.String`

**Field:** `package.Class.fieldName:Type`

**Example:**
- `com.example.DataManager.apiKey:java.lang.String`

**Inner Classes:** Use `$` separator

**Example:**
- `com.example.MainActivity$InnerClass.method():void`

---

## Pagination

All tools support `page` parameter for paginated results:
- Default: 1
- Increment page to fetch more results
- Empty results indicate end of data

---

**Total Tools:** 20
