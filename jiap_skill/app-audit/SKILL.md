# JIAP App Auditor

**Scope**: User-space Android Applications (APKs, System Apps).
**Focus**: Component security, IPC vulnerabilities, WebViews, and data storage.

## Security Analysis Framework

### Phase 1: Attack Surface Mapping

1. **Manifest Analysis**:
   - `get_app_manifest()`: Identify exported components, permissions, deep links.
   - Mark components with `android:exported="true"` as **High Priority**.

2. **Entry Point Identification**:
   - `get_main_activity()`: UI entry point.
   - `search_method("onCreate")`: Identify initialization paths.

---

## Specialized Vulnerability Workflows

### 1. Intent Redirection (Intent Parasite)
*Vulnerability: Exported component accepts Intent as extra and forwards it without validation.*

**Real-World Impact**: Malicious apps can access protected Activities/Services or perform actions on behalf of victim.

**Detection Protocol**:
1. **Locate Target**: Exported Activity/Service from manifest.
2. **Analyze Flow**:
   - `get_class_source(name)`
   - Search for: `getParcelableExtra("intent")`, `getExtra()`
3. **Verify Sink**:
   - Does it pass extra intent to `startActivity(...)`, `startService(...)`, or `sendBroadcast(...)`?
   - **Check**: Is `intent.getComponent()` validated against allowlist?
   - **Check**: Is `intent.getPackage()` restricted?

**Result Interpretation**:
- **VULNERABLE**: Intent extra used for starting components WITHOUT allowlist validation.
- **SECURE**: Either validates target component or restricts to internal package.

### 2. Deep Link & Custom Scheme Abuse
*Vulnerability: Unsafe handling of data from deep links (e.g., `myapp://data`).*

**Real-World Impact**: Phishing links can trigger XSS, steal cookies, or bypass login flows.

**Detection Protocol**:
1. **Find Handlers**:
   - Manifest: `intent-filter` with `<data android:scheme="...">`
2. **Trace Data**:
   - `get_class_source(activity)`: Read `onCreate`/`onNewIntent`
   - Follow: `getData()`, `getQueryParameter("...")`
3. **Check Sensitive Sinks**:
   - **WebView**: `webView.loadUrl(userData)` → **XSS**
   - **File IO**: `new File(userDataPath)` → **Path Traversal**
   - **SQL**: `db.rawQuery(userData)` → **SQL Injection**

**Result Interpretation**:
- **VULNERABLE**: User-controlled data reaches sensitive sink without sanitization.
- **SECURE**: Data is validated, sanitized, or restricted to safe characters.

### 3. WebView JavaScript Interface RCE
*Vulnerability: `addJavascriptInterface` exposes Java methods to untrusted web content.*

**Real-World Impact**: Malicious websites can execute Java code with app permissions.

**Detection Protocol**:
1. **Locate WebViews**:
   - `search_method("addJavascriptInterface")`
2. **Analyze Interface Object**:
   - `get_class_source(interfaceObjectClass)`
   - List all public methods.
3. **Verify Security**:
   - **Target SDK**: `targetSdkVersion < 17` → **CRITICAL RCE**
   - **Annotations**: Are methods marked `@JavascriptInterface`?
   - **Exposed Functions**: Can interface access `SharedPreferences`, `Runtime.exec()`, or `startActivity()`?
   - **File Access**: Is `setAllowFileAccess(true)` enabled?

**Result Interpretation**:
- **CRITICAL**: Target SDK < 17 or exposes dangerous methods without validation.
- **HIGH**: File access enabled with JS interfaces.
- **LOW**: Properly annotated with no sensitive APIs exposed.

### 4. Hardcoded Secrets & API Keys
*Vulnerability**: API keys, AWS credentials, or secrets embedded in bytecode.*

**Detection Protocol**:
1. **Keyword Search**:
   - `search_class_key("password")`
   - `search_class_key("api_key")`, `search_class_key("aws_access")`
   - `search_class_key("Bearer")`
2. **Check BuildConfig**:
   - `get_class_source("...BuildConfig")`: Search for `DEBUG` or `FLAVOR` leaking logic.

**Result Interpretation**:
- **Report**: List all secrets found with class location and severity.

---

## Tool Reference

| Tool | Purpose |
|-------|----------|
| `get_app_manifest()` | **START**: Map attack surface |
| `get_main_activity()` | Identify UI entry point |
| `get_class_source(name)` | Read logic |
| `get_method_xref(sig)` | Trace data flow |
| `search_class_key(key)` | Find secrets/patterns |
