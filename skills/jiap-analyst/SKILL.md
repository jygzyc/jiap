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

**Focus:** IPC vulnerabilities, exported components, WebViews, SQL injection

```python
# 1. Map attack surface
get_exported_components(1)
get_deep_links(1)
get_app_manifest(1)

# 2. Find vulnerable patterns
search_method("addJavascriptInterface", 1)  # WebView RCE
search_method("rawQuery", 1)                 # SQL injection
search_method("startActivity", 1)            # Intent redirection

# 3. Trace to sensitive sinks
get_method_xref("<signature>", 1)
```

### Framework Audit Workflow

**Focus:** Privilege escalation, Binder IPC, permission bypass

```python
# 1. Map system services
get_system_service_impl("android.os.IInterface", 1)

# 2. Check permission enforcement
search_class_key("enforceCallingPermission")
search_class_key("clearCallingIdentity")

# 3. Find identity confusion
search_method("int uid")
get_method_source("methodWithUid")
```

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
Component: com.package.ClassName.vulnerableMethod()
Cause: Brief description of the flaw

Evidence:
  Intent intent = getIntent().getParcelableExtra("key");
  startActivity(intent);  // No validation

Exploit Path:
  1. Attacker crafts malicious Intent
  2. Application receives user-controlled input
  3. Input flows to vulnerable sink without validation
  4. Attacker achieves X

Impact: What the attacker gains
Mitigation: How to fix
```

---

## References

| Reference | Content |
|-----------|---------|
| `references/app-audit.md` | App vulnerabilities: Intent, WebView, SQL injection |
| `references/framework-audit.md` | Framework vulnerabilities: Permission bypass, IPC |
| `references/tool-reference.md` | Tool usage, API documentation |
| `references/vulnerabilities.md` | Vulnerability patterns, exploitation techniques |
