# App Security Audit Workflow

**Scope**: User-space Android Applications (APKs, System Apps)
**Focus**: Component security, IPC vulnerabilities, WebViews, and data storage

## Phase 1: Attack Surface Mapping

### Step 1: Manifest Analysis

```
get_app_manifest()
```

**Extract and prioritize**:
- Components with `android:exported="true"` → **HIGH PRIORITY**
- Deep link schemes in `<intent-filter>` with `<data android:scheme="...">`
- Permissions declared and requested
- `android:debuggable`, `android:allowBackup` flags

### Step 2: Entry Point Identification

```
get_main_activity()
get_application()
search_method("onCreate")
```

Map initialization paths and identify where user data first enters the app.

---

## Vulnerability Detection Protocols

### 1. Intent Redirection (Intent Parasite)

**Description**: Exported component accepts Intent as extra and forwards it without validation.

**Real-World Impact**: Malicious apps access protected Activities/Services or perform actions on behalf of victim.

**Detection Steps**:

1. **Locate Target**: Identify exported Activity/Service from manifest
   ```
   get_app_manifest()  # Find exported="true" components
   ```

2. **Analyze Flow**:
   ```
   get_class_source("com.example.ExportedActivity")
   ```
   Search for patterns:
   - `getParcelableExtra("intent")`
   - `getExtra("intent")`
   - `getIntent().getExtras()`

3. **Verify Sink**: Check if extra Intent reaches:
   - `startActivity(intent)`
   - `startService(intent)`
   - `sendBroadcast(intent)`

4. **Check Validation**:
   - Is `intent.getComponent()` validated against allowlist?
   - Is `intent.getPackage()` restricted to internal package?

**Vulnerability Criteria**:
- **VULNERABLE**: Intent extra used for starting components WITHOUT allowlist validation
- **SECURE**: Validates target component OR restricts to internal package

**Reporting Template**:
```
[HIGH] Intent Redirection in ExportedActivity
Risk: HIGH
Component: com.example.ExportedActivity.onCreate()
Cause: Accepts untrusted Intent from extras and forwards to startActivity()
Evidence: 
  Intent target = getIntent().getParcelableExtra("redirect");
  startActivity(target);  // No validation
Impact: Attacker can access any protected Activity in victim's context
Mitigation: Validate intent.getComponent() against explicit allowlist
```

---

### 2. Deep Link & Custom Scheme Abuse

**Description**: Unsafe handling of data from deep links (e.g., `myapp://data`).

**Real-World Impact**: Phishing links trigger XSS, steal cookies, or bypass login flows.

**Detection Steps**:

1. **Find Handlers**: From manifest, locate Activities with:
   ```xml
   <intent-filter>
     <data android:scheme="myapp" />
   </intent-filter>
   ```

2. **Trace Data**:
   ```
   get_class_source("com.example.DeepLinkActivity")
   ```
   Follow data from:
   - `getData()` / `getData().toString()`
   - `getQueryParameter("param")`
   - `getPathSegments()`

3. **Check Sensitive Sinks**:

   | Sink Type | Pattern | Vulnerability |
   |-----------|---------|---------------|
   | WebView | `webView.loadUrl(userData)` | XSS |
   | File IO | `new File(userPath)` | Path Traversal |
   | SQL | `db.rawQuery(userData)` | SQL Injection |
   | Reflection | `Class.forName(userData)` | Code Execution |

**Vulnerability Criteria**:
- **VULNERABLE**: User-controlled data reaches sensitive sink without sanitization
- **SECURE**: Data validated, sanitized, or restricted to safe characters

**Reporting Template**:
```
[HIGH] Deep Link XSS in DeepLinkHandler
Risk: HIGH
Component: com.example.DeepLinkHandler.handleUrl()
Cause: URL parameter passed directly to WebView.loadUrl()
Evidence:
  String url = getIntent().getData().getQueryParameter("url");
  webView.loadUrl(url);  // No validation
Impact: Attacker-controlled URL can execute JavaScript in WebView context
Mitigation: Validate URL against allowlist of trusted domains
```

---

### 3. WebView JavaScript Interface RCE

**Description**: `addJavascriptInterface` exposes Java methods to untrusted web content.

**Real-World Impact**: Malicious websites execute Java code with app permissions.

**Detection Steps**:

1. **Locate WebViews**:
   ```
   search_method("addJavascriptInterface")
   ```

2. **Analyze Interface Object**:
   ```
   get_class_source("com.example.JsInterface")
   ```
   List all public methods in the interface class.

3. **Assess Risk Factors**:

   | Factor | Check | Risk Level |
   |--------|-------|------------|
   | Target SDK | `targetSdkVersion < 17` | **CRITICAL** (no annotation required) |
   | Annotations | Methods lack `@JavascriptInterface` | HIGH (SDK < 17) |
   | Dangerous APIs | `Runtime.exec()`, `startActivity()`, file access | HIGH |
   | File Access | `setAllowFileAccess(true)` | MEDIUM |

4. **Verify URL Loading**:
   - Does WebView load untrusted URLs?
   - Is `setJavaScriptEnabled(true)`?

**Vulnerability Criteria**:
- **CRITICAL**: Target SDK < 17 OR exposes dangerous methods without validation
- **HIGH**: File access enabled with JS interfaces
- **LOW**: Properly annotated with no sensitive APIs exposed

**Reporting Template**:
```
[CRITICAL] WebView JavaScript Interface RCE
Risk: CRITICAL
Component: com.example.MyWebView + JsInterface
Cause: addJavascriptInterface exposes file operations to untrusted content
Evidence:
  webView.addJavascriptInterface(new JsInterface(), "app");
  // JsInterface.readFile(String path) is public
Impact: Attacker-controlled website can read arbitrary files
Mitigation: Remove interface OR restrict to @JavascriptInterface annotated safe methods
```

---

### 4. Hardcoded Secrets & API Keys

**Description**: API keys, AWS credentials, or secrets embedded in bytecode.

**Detection Steps**:

1. **Keyword Search**:
   ```
   search_class_key("password")
   search_class_key("api_key")
   search_class_key("secret")
   search_class_key("aws_access")
   search_class_key("Bearer")
   search_class_key("private_key")
   ```

2. **Check BuildConfig**:
   ```
   get_class_source("com.example.BuildConfig")
   ```
   Look for `DEBUG`, `API_KEY`, or flavor-specific values.

3. **Check Common Locations**:
   - `res/values/strings.xml` (if extractable)
   - Obfuscated string arrays
   - Base64 encoded values

**Reporting Template**:
```
[MEDIUM] Hardcoded API Key
Risk: MEDIUM
Component: com.example.ApiClient
Cause: Production API key embedded in source code
Evidence:
  private static final String API_KEY = "sk-live-xxxxx";
Impact: Key extraction enables unauthorized API access
Mitigation: Move to secure storage or backend proxy
```

---

### 5. Insecure Data Storage

**Description**: Sensitive data stored without encryption.

**Detection Steps**:

1. **Search Storage APIs**:
   ```
   search_method("getSharedPreferences")
   search_method("openFileOutput")
   search_class_key("MODE_WORLD_READABLE")
   search_class_key("MODE_WORLD_WRITEABLE")
   ```

2. **Check for Sensitive Data**:
   - Credentials, tokens, PII stored in SharedPreferences
   - Files created with world-readable permissions

3. **Verify Encryption**:
   - Is `EncryptedSharedPreferences` used?
   - Is file content encrypted?

---

### 6. SQL Injection

**Description**: User input directly concatenated into SQL queries.

**Detection Steps**:

1. **Search Raw Queries**:
   ```
   search_method("rawQuery")
   search_method("execSQL")
   ```

2. **Trace Input**:
   ```
   get_method_xref("android.database.sqlite.SQLiteDatabase.rawQuery")
   ```
   Follow data from user input to query execution.

3. **Check Parameterization**:
   - Is `?` placeholder with `selectionArgs` used?
   - Or string concatenation: `"SELECT * FROM users WHERE id=" + userId`

---

## Tool Quick Reference

| Tool | Purpose | Example |
|------|---------|---------|
| `get_app_manifest()` | **START**: Map attack surface | Entry point |
| `get_main_activity()` | Identify UI entry point | Initial analysis |
| `get_application()` | App class initialization | Startup logic |
| `get_class_source(name)` | Read component logic | Deep analysis |
| `get_method_xref(sig)` | Trace data flow | Sink verification |
| `search_class_key(key)` | Find secrets/patterns | Keyword hunting |
| `search_method(name)` | Find API usage | Vulnerability patterns |

## Analysis Checklist

- [ ] Manifest analyzed, exported components identified
- [ ] Entry points mapped (MainActivity, Application)
- [ ] Intent handling reviewed for redirection
- [ ] Deep link handlers checked for injection
- [ ] WebView configurations audited
- [ ] Hardcoded secrets searched
- [ ] Data storage reviewed for encryption
- [ ] SQL queries checked for injection
