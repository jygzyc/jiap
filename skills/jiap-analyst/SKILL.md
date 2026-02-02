---
name: jiap-analyst
description: This skill should be used when the user asks to "analyze Android APK", "audit Android app security", "reverse engineer Android", "analyze Android framework", "find Android vulnerabilities", "security audit with JADX", "use JIAP tools", or mentions JIAP, JADX security analysis, Android reverse engineering, APK security, or framework vulnerability research.
version: 1.0.0
---

# JIAP Analyst

Android reverse engineering and security analysis skill using **JIAP (Java Intelligence Analysis Platform)**. Routes to specialized workflows for App (APK) security auditing or Framework (ROM) vulnerability research.

## Prerequisites

Ensure JIAP plugin is active:
1. Run `health_check()` to verify connection
2. Confirm JADX has loaded target file (APK, JAR, or DEX)

## Target Type Routing (Mandatory First Step)

Before analysis, determine the target type to select the correct workflow:

| Target Type | Package Indicators | Action |
|-------------|-------------------|--------|
| **User Application (APK)** | `com.facebook.*`, `com.google.android.gms`, `com.tencent.mm`, third-party packages | Use **App Audit Workflow** (`references/app-audit.md`) |
| **Android Framework (ROM)** | `com.android.server.*`, `android.os.IPowerManager`, `com.android.internal.*` | Use **Framework Audit Workflow** (`references/framework-audit.md`) |

### Discovery Protocol (If Target Unknown)

1. `get_all_classes(page=1)` - Inspect package names
   - `com.android.server.*` → Framework
   - Third-party names → App
2. `get_app_manifest()` - Check for manifest
   - Contains `<manifest>` / `<application>` → App or System App
   - Empty/Error → Likely Framework JAR

## Core Tool Categories

| Category | Tools | Purpose |
|----------|-------|---------|
| **Code Retrieval** | `get_class_source`, `get_method_source` | Read Java/Smali code |
| **Search** | `search_class_key`, `search_method` | Find patterns/keywords |
| **Structure** | `get_class_info`, `get_sub_classes`, `get_implement` | Class hierarchies |
| **Cross-Reference** | `get_method_xref`, `get_class_xref` | Trace usage/data flow |
| **Android-Specific** | `get_app_manifest`, `get_main_activity`, `get_application`, `get_system_service_impl` | Android analysis |
| **UI Integration** | `selected_text`, `selected_class` | JADX GUI interaction |

For complete tool documentation, see `references/tool-reference.md`.

## Analysis Best Practices

1. **Context First**: Ground analysis in manifest (Apps) or interface mapping (Framework)
2. **Pagination Awareness**: Check `page` parameter; results may span multiple pages
3. **Evidence-Based Reporting**: Cite specific code with method signatures
4. **Risk Classification**: High/Medium/Low with exploitability reasoning

## Workflow Quick Reference

### App Security Audit (APK)

**Focus**: Component security, IPC vulnerabilities, WebViews, data storage

1. **Map Attack Surface**: `get_app_manifest()` → identify exported components
2. **Analyze Entry Points**: `get_main_activity()`, `get_class_source(activity)`
3. **Search Vulnerabilities**:
   - Intent Redirection: Look for `getParcelableExtra("intent")` + `startActivity()`
   - WebView RCE: `search_method("addJavascriptInterface")`
   - Secrets: `search_class_key("api_key")`, `search_class_key("password")`
4. **Trace Data Flow**: `get_method_xref()` for sensitive sinks

Detailed protocols: `references/app-audit.md`

### Framework Vulnerability Research (ROM)

**Focus**: Privilege escalation, Binder IPC security, denial-of-service

1. **Map Interface to Implementation**: `get_system_service_impl("android.os.IInterface")`
2. **Analyze Permission Checks**: Look for `enforceCallingPermission()` ordering
3. **Check Identity Validation**: Verify `Binder.getCallingUid()` usage vs trusted `int uid` args
4. **Find Side Channels**: Inconsistent error handling paths

Detailed protocols: `references/framework-audit.md`

## Reporting Template

```
[SEVERITY] Finding Title
Risk: HIGH/MEDIUM/LOW
Component: class.method() or component name
Cause: Brief description of the vulnerability
Evidence: Code snippet or method signature
Impact: What an attacker can achieve
Mitigation: Recommended fix
```

## Additional Resources

### Reference Files

- **`references/app-audit.md`** - Complete APK security audit workflows
- **`references/framework-audit.md`** - Complete framework vulnerability research protocols
- **`references/tool-reference.md`** - Detailed tool documentation with examples
