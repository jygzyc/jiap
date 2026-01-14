---
name: jiap-analyst
description: Android Reverse Engineering & Security Analysis skill using JIAP (JADX + MCP). Routes to specialized workflows for App (APK) security auditing or Framework (ROM) vulnerability research.
---

# JIAP Analyst

Your role is to analyze Android codebases using **JIAP (Java Intelligence Analysis Platform)**. Before diving into analysis, **determine the target type** to select the correct specialized workflow.

## ⚠ Routing Decision (Mandatory)

| Target Type | Package Indicators | Action |
|-------------|-------------------|--------|
| **User Application (APK)** | `com.facebook.*`, `com.google.android.gms`, `com.tencent.mm` | → **Use [App Audit Workflow](app-audit/SKILL.md)** |
| **Android Framework (ROM)** | `com.android.server.*`, `android.os.IPowerManager`, `com.android.internal.*` | → **Use [Framework Audit Workflow](framework-audit/SKILL.md)** |

### Discovery Protocol
Use if target type is unknown:

1. `get_all_classes(page=1)`: Inspect package names.
   - `com.android.server` → Framework
   - Third-party names → App
2. `get_app_manifest()`:
   - Contains `manifest`/`application` tag → App/System App
   - Empty/Error → Likely Framework (jar)

---

## 1. App Audit Context (APK)

**Scope**: Third-party applications installed via APK.
**Entry Point**: Exported components, deep links, web views.

**Common Vulnerabilities**:
- **Intent Redirection**: Exported Activity/Service forwarding untrusted Intents.
- **Deep Link XSS**: Unsafe data from URL schemes.
- **WebView RCE**: Exposed Java interfaces via `addJavascriptInterface`.
- **Hardcoded Secrets**: API keys embedded in code.

**Tool Focus**:
- `get_app_manifest()` - Map to attack surface.
- `get_class_source()` - Analyze exported components.
- `search_class_key()` - Find secrets.

---

## 2. Framework Audit Context (ROM/System)

**Scope**: Android OS services, framework code, system_server.
**Entry Point**: Binder APIs (`IPowerManager`, `IActivityManager`).

**Common Vulnerabilities**:
- **Permission Bypass**: Missing `enforceCallingPermission` checks.
- **Identity Confusion**: Trusting user-supplied `uid` vs `Binder.getCallingUid()`.
- **Side Channel**: Error oracles leaking package/process states.
- **System DoS**: Unbounded loops in privileged code.

**Tool Focus**:
- `get_system_service_impl(interface)` - Bridge interface to implementation.
- `get_method_source(signature)` - Deep inspection.
- `get_method_xref(signature)` - Trace usage.

---

## Shared Tool Reference

| Category | Tool | Usage |
|-----------|-------|--------|
| **Retrieval** | `get_class_source`, `get_method_source` | Read code (Java/Smali) |
| **Search** | `search_class_key`, `search_method` | Find patterns |
| **Structure** | `get_sub_classes`, `get_implement` | Hierarchies |
| **Cross-Ref** | `get_method_xref`, `get_class_xref` | Trace flow |
| **System** | `health_check`, `get_app_manifest`, `get_system_service_impl` | Metadata |

## Best Practices

1. **Context First**: Always ground analysis in manifest (Apps) or interface mapping (Framework).
2. **Pagination**: Check for `page` parameters; results may span multiple pages.
3. **Evidence-Based**: Cite specific code sections when reporting vulnerabilities.
4. **Risk Assessment**: Classify findings as High/Medium/Low with exploitability reasoning.
