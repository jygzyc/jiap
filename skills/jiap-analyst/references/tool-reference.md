# JIAP Tool Reference

Complete documentation for all JIAP MCP tools available for Android reverse engineering and security analysis.

## System Tools

### health_check()

**Purpose**: Verify JIAP server connection and status.

**Parameters**: None

**Returns**: Server status object

**Usage**:
```
health_check()
```

**Response Example**:
```json
{
  "status": "running",
  "url": "http://127.0.0.1:25420",
  "port": 25420,
  "timestamp": 12345678
}
```

**When to Use**: Always run first to verify connection before analysis.

---

## Code Retrieval Tools

### get_all_classes(page)

**Purpose**: Retrieve all available classes in the decompiled project.

**Parameters**:
| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `page` | int | 1 | Page number for pagination |

**Returns**: List of fully qualified class names

**Usage**:
```
get_all_classes(page=1)
```

**When to Use**: Initial reconnaissance to understand project structure and determine target type (App or Framework).

---

### get_class_source(class_name, smali, page)

**Purpose**: Retrieve the source code of a specific class.

**Parameters**:
| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `class_name` | string | required | Full class name (e.g., `com.example.MyClass$InnerClass`) |
| `smali` | bool | false | Return Smali format instead of Java |
| `page` | int | 1 | Page number for pagination |

**Usage**:
```
get_class_source("com.example.MainActivity")
get_class_source("com.example.MainActivity", smali=True)
```

**When to Use**: Deep analysis of component logic, vulnerability verification.

**Notes**:
- Use `$` for inner classes: `OuterClass$InnerClass`
- Smali format useful for obfuscated code or precise bytecode analysis

---

### get_method_source(method_name, smali, page)

**Purpose**: Retrieve source code of a specific method.

**Parameters**:
| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `method_name` | string | required | Full method signature |
| `smali` | bool | false | Return Smali format |
| `page` | int | 1 | Page number for pagination |

**Method Signature Format**:
```
package.Class.methodName(param1Type, param2Type):returnType
```

**Examples**:
```
get_method_source("com.example.Utils.decrypt(java.lang.String):java.lang.String")
get_method_source("com.example.Api.sendRequest(byte[], int):void")
```

**When to Use**: Focused analysis on specific methods, especially after finding via search.

---

### get_class_info(class_name, page)

**Purpose**: Get class metadata including fields and methods.

**Parameters**:
| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `class_name` | string | required | Full class name |
| `page` | int | 1 | Page number for pagination |

**Returns**: Object containing:
- Class name
- Class field signatures (name, type, modifiers)
- Class method signatures (name, parameters, return type)

**Usage**:
```
get_class_info("com.example.DataManager")
```

**When to Use**: Enumerate class structure before deep analysis.

---

## Search Tools

### search_method(method_name, page)

**Purpose**: Search for methods matching a name pattern.

**Parameters**:
| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `method_name` | string | required | Method name or partial name |
| `page` | int | 1 | Page number for pagination |

**Returns**: List of matching method signatures

**Usage**:
```
search_method("decrypt")
search_method("addJavascriptInterface")
search_method("startActivity")
```

**When to Use**: Find vulnerability patterns across the entire codebase.

**Common Security Searches**:
| Pattern | Purpose |
|---------|---------|
| `addJavascriptInterface` | WebView RCE |
| `getParcelableExtra` | Intent redirection |
| `rawQuery` | SQL injection |
| `loadUrl` | WebView injection |
| `enforceCallingPermission` | Permission checks |
| `clearCallingIdentity` | Binder identity handling |
| `startActivity` | Launch any where |

---

### search_class_key(key, page)

**Purpose**: Search for classes containing a keyword in their source code.

**Parameters**:
| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `key` | string | required | Keyword to search (case-insensitive) |
| `page` | int | 1 | Page number for pagination |

**Returns**: List of classes containing the keyword

**Usage**:
```
search_class_key("password")
search_class_key("api_key")
search_class_key("MODE_WORLD_READABLE")
```

**When to Use**: Find hardcoded secrets, specific patterns, or vulnerability indicators.

**Common Security Searches**:
| Keyword | Purpose |
|---------|---------|
| `password`, `secret`, `api_key` | Hardcoded credentials |
| `Bearer`, `Authorization` | Token exposure |
| `aws_access`, `private_key` | Cloud credentials |
| `MODE_WORLD_READABLE` | Insecure file permissions |
| `setAllowFileAccess` | WebView file access |

---

## Cross-Reference Tools

### get_method_xref(method_name, page)

**Purpose**: Find all locations where a method is called.

**Parameters**:
| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `method_name` | string | required | Full method signature |
| `page` | int | 1 | Page number for pagination |

**Returns**: List of call sites (caller class, method, and location)

**Usage**:
```
get_method_xref("com.example.Crypto.decrypt(java.lang.String):java.lang.String")
```

**When to Use**: 
- Trace data flow from source to sink
- Understand how a vulnerable method is reached
- Find all callers of security-critical functions

---

### get_class_xref(class_name, page)

**Purpose**: Find all locations where a class is used.

**Parameters**:
| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `class_name` | string | required | Full class name |
| `page` | int | 1 | Page number for pagination |

**Returns**: List of usage locations

**Usage**:
```
get_class_xref("com.example.SecurityManager")
```

**When to Use**: Understand class usage patterns and dependencies.

---

## Structure Tools

### get_sub_classes(class_name, page)

**Purpose**: Find all subclasses of a given class.

**Parameters**:
| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `class_name` | string | required | Full superclass name |
| `page` | int | 1 | Page number for pagination |

**Returns**: List of subclass names

**Usage**:
```
get_sub_classes("android.app.Activity")
get_sub_classes("com.android.server.SystemService")
```

**When to Use**: Find all implementations of a base class (e.g., all Activities, all Services).

---

### get_implement(interface_name, page)

**Purpose**: Find all classes implementing an interface.

**Parameters**:
| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `interface_name` | string | required | Full interface name |
| `page` | int | 1 | Page number for pagination |

**Returns**: List of implementing class names

**Usage**:
```
get_implement("android.os.IInterface")
get_implement("com.example.ICallback")
```

**When to Use**: Find all implementations of an AIDL interface.

---

## Android-Specific Tools

### get_app_manifest(page)

**Purpose**: Retrieve the AndroidManifest.xml content.

**Parameters**:
| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `page` | int | 1 | Page number for pagination |

**Returns**: Parsed manifest content

**Usage**:
```
get_app_manifest()
```

**When to Use**: **ALWAYS first** for APK analysis to map attack surface.

**Key Elements to Extract**:
- `android:exported="true"` components
- Deep link schemes in `<intent-filter>`
- Permissions declared and used
- `android:debuggable`, `android:allowBackup` flags
- Content providers and their permissions

---

### get_main_activity(page)

**Purpose**: Get the main activity (launcher entry point).

**Parameters**:
| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `page` | int | 1 | Page number for pagination |

**Returns**: Main activity class name and source

**Usage**:
```
get_main_activity()
```

**When to Use**: Identify app entry point for analysis starting point.

---

### get_application(page)

**Purpose**: Get the Application class and its information.

**Parameters**:
| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `page` | int | 1 | Page number for pagination |

**Returns**: Application class details

**Usage**:
```
get_application()
```

**When to Use**: Analyze app initialization, often contains important setup code.

---

### get_system_service_impl(interface_name, page)

**Purpose**: Map an AIDL interface to its system service implementation.

**Parameters**:
| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `interface_name` | string | required | Full interface name (e.g., `android.os.IPowerManager`) |
| `page` | int | 1 | Page number for pagination |

**Returns**: Implementation class name and details

**Usage**:
```
get_system_service_impl("android.os.IPowerManager")
get_system_service_impl("android.app.IActivityManager")
```

**When to Use**: **PRIMARY tool** for framework analysis - bridges AIDL interfaces to implementations.

**Common Mappings**:
| Interface | Implementation |
|-----------|----------------|
| `android.os.IPowerManager` | `PowerManagerService` |
| `android.app.IActivityManager` | `ActivityManagerService` |
| `android.content.pm.IPackageManager` | `PackageManagerService` |

---

## UI Integration Tools

### selected_text(page)

**Purpose**: Get currently selected text in JADX GUI.

**Parameters**:
| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `page` | int | 1 | Page number for pagination |

**Returns**: Selected text content

**Usage**:
```
selected_text()
```

**When to Use**: Interactive analysis with JADX GUI.

---

### selected_class(page)

**Purpose**: Get currently selected class in JADX GUI.

**Parameters**:
| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `page` | int | 1 | Page number for pagination |

**Returns**: Selected class information

**Usage**:
```
selected_class()
```

**When to Use**: Quick context when working with JADX GUI.

---

## Pagination

All tools support pagination via the `page` parameter:

- Default page size varies by tool
- Start with `page=1`
- Increment page number to retrieve more results
- Empty results indicate end of data

**Example**:
```
get_all_classes(page=1)  # First batch
get_all_classes(page=2)  # Second batch
```

---

## Error Handling

All tools return structured error responses:

```json
{
  "error": "E015",
  "message": "Method not found: com.example.Missing.method()"
}
```

**Common Error Codes**:
| Code | Description | Resolution |
|------|-------------|------------|
| E014 | Health check failed | Verify JIAP server is running |
| E015 | Method not found | Check method signature format |
| E016 | Missing parameter | Provide required parameters |
| E017 | Invalid parameter | Check parameter format/value |
| E019 | Connection error | Verify network/server status |

---

## Tool Selection Guide

| Task | Primary Tool | Supporting Tools |
|------|--------------|------------------|
| Initial recon | `get_all_classes` | `get_app_manifest` |
| Map attack surface | `get_app_manifest` | `get_main_activity` |
| Find patterns | `search_method`, `search_class_key` | |
| Analyze code | `get_class_source`, `get_method_source` | `get_class_info` |
| Trace flow | `get_method_xref`, `get_class_xref` | |
| Find implementations | `get_implement`, `get_sub_classes` | `get_system_service_impl` |
| Framework analysis | `get_system_service_impl` | `get_method_source` |
