---
name: jiap-analyst
description: Use when user asks to "analyze Android APK", "audit Android app security", "reverse engineer Android", "analyze Android framework", "find Android vulnerabilities", "security audit with JADX", or mentions JIAP, JADX security analysis, Android reverse engineering, APK security, or framework vulnerability research.
---

# JIAP Analyst

Android reverse engineering and security analysis using JADX and JIAP plugin.

## Prerequisites

### Step 1: Environment Check

Use `jadx_check.py` to verify JADX and JIAP are available:

```bash
python scripts/jadx_check.py --help
python scripts/jadx_check.py
python scripts/jadx_check.py --install  # Install if missing
```

**Checks:** JADX binary, JIAP plugin, JIAP server health endpoint

---

### Step 2: Decompile Target

Use `jadx_analyze.py` to decompile APK, DEX, JAR, or AAR files:

```bash
python scripts/jadx_analyze.py --help
python scripts/jadx_analyze.py /path/to/app.apk
python scripts/jadx_analyze.py /path/to/app.apk --output ./analysis --threads 8
```

**Output:** `<filename>_jadx/` directory with decompiled Java code

---

## Security Audit

After decompilation, load the appropriate reference and start analysis:

### Target Type Routing

| Target Type | Package Indicators | Load Reference |
|-------------|-------------------|----------------|
| **User Application (APK)** | Third-party packages (com.facebook.*, etc.) | `references/app-audit.md` |
| **Android Framework (ROM)** | System packages (com.android.server.*) | `references/framework-audit.md` |

### App Audit Workflow

**Focus:** User-space application vulnerabilities - IPC, exported components, WebViews, data flow analysis

```python
# 1. Identify exported components (entry points)
get_exported_components(page=1)
get_deep_links(page=1)
get_dynamic_receivers(page=1)

# 2. Extract Intent data sources
search_method(method_name="getIntent", page=1)
search_method(method_name="getStringExtra", page=1)
search_method(method_name="getParcelableExtra", page=1)

# 3. Trace taint propagation to sinks (use full method signature)
get_method_xref(method_name="com.app.Activity.onCreate(android.os.Bundle):void", page=1)
get_method_source(method_name="com.app.Helper.processData(java.lang.String):void", page=1)

# 4. Check for sanitizers
search_class_key(keyword="ALLOWED", page=1)
search_method(method_name="getCanonicalPath", page=1)
```

**Key Analysis Targets:**
- **Activity**: `getIntent()` data flow via cross-reference
- **Service**: AIDL interface implementation analysis
- **ContentProvider**: `call()` function and file operations

### Framework Audit Workflow

**Focus:** System-level vulnerabilities - privilege escalation, Binder IPC, permission bypass

```python
# 1. Map system services
get_system_service_impl(interface_name="android.os.IPowerManager", page=1)

# 2. Check permission enforcement
search_class_key(keyword="enforceCallingPermission", page=1)
search_class_key(keyword="clearCallingIdentity", page=1)

# 3. Find identity confusion
search_method(method_name="processForUser", page=1)
get_method_source(method_name="com.android.server.Service.processForUser(int,java.lang.String):void", page=1)

# 4. Check for data exposure
search_method(method_name="getRunningAppProcesses", page=1)
search_method(method_name="getRecentTasks", page=1)
```

**Key Analysis Targets:**
- **Permission Bypass**: Missing or misordered permission checks
- **Identity Confusion**: Trusting `uid` parameter without `Binder.getCallingUid()` validation
- **Data Exposure**: Sensitive data returned without proper permission
- **Side Channels**: Information leak through different return paths

---

## Core Principle: Exploitability Only

**Report ONLY findings with demonstrable exploitability.**

A finding is a vulnerability **ONLY if** all 3 conditions are met:
1. **Reachable** - Attacker can trigger the code path
2. **Controllable** - Attacker can influence critical data
3. **Impactful** - Causes security breach (privilege escalation, data theft, RCE, DoS)

**If ANY condition is NOT met → Do NOT report.**

---

## Reporting Template

```
[SEVERITY] Vulnerability Title
Risk: CRITICAL/HIGH/MEDIUM/LOW
Component: com.package.ClassName.vulnerableMethod(paramType):returnType
Cause: Brief description of the flaw

Evidence:
  // Source: getIntent() receives attacker-controlled data
  Intent intent = getIntent().getParcelableExtra("key");
  
  // Sink: Reaches startActivity() without validation
  startActivity(intent);  // No allowlist check

Taint Flow:
  Source: getIntent().getParcelableExtra("forward_intent")
    ↓
  Propagation: Helper.processIntent() stores in field
    ↓
  [NO SANITIZER] - No allowlist validation found
    ↓
  Sink: startActivity() launches arbitrary component

Exploit Path:
  1. Attacker crafts malicious Intent with internal component
  2. Send to exported Activity via adb: adb shell am start -n com.app/.ExportedActivity --es forward_intent "..."
  3. Activity extracts forward_intent without validation
  4. startActivity() launches privileged internal component
  5. Attacker gains unauthorized access

Impact: What the attacker gains (e.g., launch arbitrary components, privilege escalation)
Mitigation: Implement allowlist validation before startActivity()
```

---

## References

| Reference | Content | When to Use |
|-----------|---------|-------------|
| `references/app-audit.md` | **App Audit Methodology**: How to analyze Activity/Service/ContentProvider for user applications | Target is APK file, third-party app |
| `references/framework-audit.md` | **Framework Audit Methodology**: How to analyze Android system services | Target is framework.jar, system services |
| `references/vulnerabilities.md` | **Vulnerability Catalog**: Complete list of App & Framework vulnerability types with detection patterns | Need to understand specific vulnerability patterns |
| `references/tool-reference.md` | **Tool Reference**: Complete MCP tool API documentation with parameter formats | Need exact tool signatures or parameter details |

### Tool Signature Format

All tools use **named parameters** with `page` as the last parameter:

```python
# Search tools: (search_term, page)
search_method(method_name="startActivity", page=1)
search_class_key(keyword="ALLOWED", page=1)

# Retrieval tools: (identifier, page)
get_method_source(method_name="com.app.Activity.onCreate(android.os.Bundle):void", page=1)
get_class_info(class_name="com.app.Helper", page=1)
get_method_xref(method_name="com.app.Activity.onCreate(android.os.Bundle):void", page=1)

# Android-specific: (page) or (interface_name, page)
get_exported_components(page=1)
get_deep_links(page=1)
get_system_service_impl(interface_name="android.os.IPowerManager", page=1)
```

**Method Signature Format:** `package.Class.methodName(paramType1,paramType2):returnType`

**Examples:**
- `com.app.MainActivity.onCreate(android.os.Bundle):void`
- `com.app.Helper.processData(java.lang.String):void`
- `com.android.server.Service.processForUser(int,java.lang.String):void`
