# Vulnerability Patterns

Quick reference for common Android security vulnerabilities.

## Taint Tracking Fundamentals

**Vulnerability = Untrusted Data (Source) → Exploitable Sink without Sanitization**

### Sources (Untrusted Data Entry Points)

Data from these sources is **tainted** and potentially attacker-controlled:

| Source | API Pattern | Verification |
|--------|-------------|--------------|
| **Intent Extras** | `getIntent().getStringExtra()`<br>`getParcelableExtra()`<br>`getExtras()` | Exported component? Deep link? |
| **Deep Link Data** | `getIntent().getData()`<br>`getQueryParameter()`<br>`getPathSegments()` | URL scheme exposed? |
| **Content Provider** | `query()`, `insert()`, `update()` | `android:exported="true"`? |
| **Broadcast Receiver** | `onReceive(Intent)` | Receiver exported? Dynamic? |
| **WebView Input** | `addJavascriptInterface()`<br>`shouldOverrideUrlLoading()` | Loads untrusted content? |
| **File/Network** | `openFileInput()`<br>`getInputStream()`<br>`SharedPreferences` (world-readable) | File world-readable? Network HTTP? |
| **System Service** | `TelephonyManager.getDeviceId()`<br>`getSubscriberId()` | Sensitive data leak? |

### Sinks (Dangerous Execution Points)

Tainted data reaching these sinks is exploitable:

| Sink | API Pattern | Exploit |
|------|-------------|---------|
| **Component Launch** | `startActivity(Intent)`<br>`startService(Intent)`<br>`sendBroadcast(Intent)` | Intent redirection, launch any component |
| **Command Execution** | `Runtime.exec()`<br>`ProcessBuilder.start()` | Arbitrary command execution |
| **SQL Query** | `rawQuery(sql)`<br>`execSQL(sql)` | SQL injection |
| **File Access** | `FileInputStream(path)`<br>`FileOutputStream(path)`<br>`openFileOutput(name)` | Path traversal |
| **WebView Load** | `loadUrl(url)`<br>`loadData(data)` | XSS, phishing, data exfiltration |
| **Reflection** | `Class.forName(name)`<br>`getMethod(name).invoke()` | Code execution, bypass |
| **Native Code** | `System.loadLibrary()`<br>`Runtime.load()` | Library hijacking |

### Taint Propagation Patterns

Track how tainted data flows through the code:

```java
// Direct flow (taint preserved)
String tainted = getIntent().getStringExtra("key");  // Source
String stillTainted = tainted.toLowerCase();         // Propagation
executeCommand(stillTainted);                        // Sink - VULNERABLE

// Indirect flow through fields (taint preserved)
this.mUserInput = getIntent().getStringExtra("key"); // Source
processLater();                                      // Method call
void processLater() {
    executeCommand(this.mUserInput);                 // Sink - VULNERABLE
}

// Collection flow (taint preserved)
List<String> list = new ArrayList<>();
list.add(getIntent().getStringExtra("key"));         // Source
String extracted = list.get(0);
executeCommand(extracted);                           // Sink - VULNERABLE

// Intent propagation (taint may spread)
Intent intent = new Intent();
intent.putExtra("data", getIntent().getStringExtra("key"));  // Source
targetActivity.processIntent(intent);                // Taint transferred

// String concatenation (taint preserved)
String base = "SELECT * FROM users WHERE name = '";
String query = base + getIntent().getStringExtra("name");  // Source
rawQuery(query, null);                               // Sink - VULNERABLE
```

### Sanitizers (Data Validation/Cleansing)

These patterns **break** taint propagation (if properly implemented):

| Sanitizer | Validation Pattern | Must Verify |
|-----------|-------------------|-------------|
| **Allowlist Check** | `ALLOWED.contains(input)`<br>`input.equals("expected")` | Complete validation before sink |
| **Type Conversion** | `Integer.parseInt(input)`<br>`Boolean.parseBoolean(input)` | Non-exploitable conversion |
| **Path Normalization** | `file.getCanonicalPath()`<br>`.startsWith(baseDir)` | Canonical path within bounds |
| **Intent Component Restriction** | `intent.setComponent(knownSafe)`<br>`intent.setPackage(knownPackage)` | Component explicitly locked |
| **SQL Parameterization** | `rawQuery("...?", args)` | All user input in args |

**Important:** Partial sanitization is NOT sufficient
```java
// ❌ NOT a sanitizer (partial)
if (!input.contains(";")) {
    execSQL("SELECT * FROM t WHERE id = '" + input + "'");  // Still vulnerable
}

// ✅ Valid sanitizer
execSQL("SELECT * FROM t WHERE id = ?", new String[]{input});
```

### Taint Analysis Workflow

For each vulnerability type:

1. **Identify Sources**: Find all places where attacker-controlled data enters
   ```
   search_method("getStringExtra")
   search_method("getData")
   search_method("getQueryParameter")
   ```

2. **Trace Forward**: Follow data flow to potential sinks
   ```
   get_method_xref("startActivity")      // Who calls startActivity?
   get_method_source("suspectMethod")    // Read the method code
   ```

3. **Check Sanitization**: Verify if data is validated before reaching sink
   - Is there an allowlist check?
   - Is the path canonicalized?
   - Are SQL queries parameterized?

4. **Validate Exploitability**:
   - Source is actually reachable by attacker
   - No effective sanitizer in the path
   - Sink is actually exploitable

### Common Taint Tracking Commands

```bash
# Find all sources
search_method("getIntent", 1)
search_method("getData", 1)
search_method("getQueryParameter", 1)
search_method("getParcelableExtra", 1)
get_exported_components(1)              # Check if sources are exposed
get_deep_links(1)                       # Check deep link entry points

# Find all sinks
search_method("startActivity", 1)
search_method("Runtime.getRuntime", 1)
search_method("rawQuery", 1)
search_method("execSQL", 1)
search_method("loadUrl", 1)

# Trace data flow
get_method_xref("suspectedMethod", 1)
get_class_source("className", 1)
search_class_key("taintedVariableName")

# Verify sanitization
search_class_key("ALLOWED")
search_class_key("contains")
search_method("getCanonicalPath", 1)
search_class_key("Parameterized")
```

### Verification Checklist

Before reporting any vulnerability, verify:

- [ ] **Source confirmed**: Data comes from attacker-controlled entry point
- [ ] **Path traced**: Tainted data flows to sink (use `get_method_source`, `get_method_xref`)
- [ ] **No sanitizer**: No allowlist, canonicalization, or parameterization in the path
- [ ] **Reachable**: Attacker can actually trigger this code path (exported component, deep link, etc.)
- [ ] **Impact confirmed**: Exploiting this causes security breach

**If ANY check fails → DO NOT REPORT**

---

## Intent Redirection (Critical)

Unvalidated Intent extras used to start activities/services without verification.

**Vulnerable:**
```java
Intent forwarded = getIntent().getParcelableExtra("forward_intent");
startActivity(forwarded);  // No validation
```

**Secure:**
```java
ComponentName target = forwarded.getComponent();
if (ALLOWED_COMPONENTS.contains(target.getClassName())) {
    startActivity(forwarded);
}
```

**Detect:** `search_method("getParcelableExtra")` + `get_method_xref("startActivity")`

---

## WebView RCE (Critical)

JavaScript interfaces expose dangerous APIs without proper restrictions.

**Vulnerable (SDK < 17):**
```java
webView.addJavascriptInterface(new Object() {
    @JavascriptInterface
    public String executeCommand(String cmd) {
        Runtime.getRuntime().exec(cmd);
        return "Done";
    }
}, "androidInterface");
```

**Secure:**
```java
// Only expose safe read-only methods
webView.addJavascriptInterface(new Object() {
    @JavascriptInterface
    public String getDeviceInfo() {
        return Build.MODEL;  // Safe, read-only
    }
}, "safeBridge");
```

**Detect:** `search_method("addJavascriptInterface")`

---

## Exported Components Without Permission (Critical)

Components exposed without proper permission protection.

**Vulnerable Manifest:**
```xml
<activity android:name=".InternalActivity"
          android:exported="true">  <!-- No permission! -->
```

**Secure Manifest:**
```xml
<activity android:name=".SensitiveActivity"
          android:exported="true"
          android:permission="com.app.permission.INTERNAL_ACCESS" />
```

**Detect:** `get_exported_components()` and check for missing `android:permission`

---

## Deep Link Hijacking (High)

Deep links handled without validating source or parameters.

**Vulnerable:**
```java
Uri uri = getIntent().getData();
String action = uri.getQueryParameter("action");
if ("delete".equals(action)) {
    deleteAccount(uri.getQueryParameter("target"));  // No validation
}
```

**Secure:**
```java
if (!"payment.victim.com".equals(uri.getHost())) {
    finish();  // Reject invalid host
    return;
}
if (!ALLOWED_ACTIONS.contains(action)) {
    finish();  // Reject invalid action
    return;
}
```

**Detect:** `get_deep_links()` and check for parameter validation

---

## SQL Injection (Critical)

SQL queries constructed using unvalidated user input.

**Vulnerable:**
```java
String query = "SELECT * FROM users WHERE username = '" + username + "'";
Cursor cursor = db.rawQuery(query, null);
```

**Secure:**
```java
String query = "SELECT * FROM users WHERE username = ?";
Cursor cursor = db.rawQuery(query, new String[]{username});
```

**Detect:** `search_method("rawQuery")` and check for string concatenation

---

## Path Traversal (High)

File paths constructed using unvalidated user input.

**Vulnerable:**
```java
public File getFile(String filename) {
    String path = "/data/data/com.app/files/" + filename;
    return new File(path);  // No validation
}
```

**Secure:**
```java
if (filename.contains("..") || filename.startsWith("/")) {
    throw new SecurityException("Invalid filename");
}
File file = new File(getFilesDir(), filename);
if (!file.getCanonicalPath().startsWith(getFilesDir().getCanonicalPath())) {
    throw new SecurityException("Path traversal blocked");
}
```

**Detect:** `search_method("File(")` and check for path construction

---

## Dynamic Receiver Registration (High)

BroadcastReceivers registered at runtime without proper validation.

**Vulnerable:**
```java
IntentFilter filter = new IntentFilter("com.app.INTERNAL_ACTION");
registerReceiver(new InternalReceiver(), filter);  // No permission!
```

**Secure:**
```java
registerReceiver(new InternalReceiver(), filter,
                 "com.app.permission.RECEIVER_INTERNAL", null);

// Or verify in onReceive:
if (checkCallingPermission("com.app.permission.INTERNAL") 
    != PackageManager.PERMISSION_GRANTED) {
    return;  // Reject unauthorized
}
```

**Detect:** `search_method("registerReceiver")` and check for permission parameter

---

## Permission Bypass (Framework)

Framework privileged operation missing permission enforcement.

**Vulnerable:**
```java
public void privilegedOperation() {
    // Missing permission check!
    clearCallingIdentity();  // Clears caller's identity
    performPrivilegedAction();  // No verification
}
```

**Secure:**
```java
public void privilegedOperation() {
    enforceCallingPermission(MY_PERMISSION);  // Check FIRST
    clearCallingIdentity();  // Then clear identity
    performPrivilegedAction();
}
```

**Detect:** `search_class_key("clearCallingIdentity")` and verify permission check before it

---

## Identity Confusion (Framework)

Trusting `int uid` parameter without validating against `Binder.getCallingUid()`.

**Vulnerable:**
```java
public void processForUser(int uid, String data) {
    // Trusts uid parameter without verification
    if (isSystemUser(uid)) {
        processPrivileged(data);
    }
}
```

**Secure:**
```java
public void processForUser(int requestedUid, String data) {
    int actualUid = Binder.getCallingUid();
    if (actualUid != requestedUid) {
        throw new SecurityException("UID mismatch");
    }
    // Process with verified UID
}
```

**Detect:** `search_method("int uid")` and check for `Binder.getCallingUid()` validation

---

## Quick Audit Commands

```bash
# Initial recon
curl http://localhost:25419/health
get_all_classes(50)
get_app_manifest(1)
get_exported_components(1)
get_deep_links(1)

# Vulnerability scanning
search_method("addJavascriptInterface", 1)   # WebView RCE
search_method("startActivity", 1)            # Intent redirection
search_method("rawQuery", 1)                 # SQL injection
search_method("getParcelableExtra", 1)       # Intent extras
search_method("registerReceiver", 1)         # Dynamic receivers
search_class_key("clearCallingIdentity")     # Permission bypass

# Trace flows
get_method_xref("startActivity", 1)
get_method_xref("execSQL", 1)
```

---

## Severity Reference

| Severity | Vulnerability Types |
|----------|-------------------|
| **Critical** | Intent Redirection, WebView RCE, SQL Injection, Permission Bypass |
| **High** | Deep Link Hijacking, Path Traversal, Dynamic Receivers, Identity Confusion |
| **Medium** | Exported Components (with limited impact), Insecure Crypto |
| **Low** | Hardcoded Secrets (requires reverse engineering), Insecure Logging |

**Note:** Only report vulnerabilities that are **Reachable**, **Controllable**, and **Impactful**.
