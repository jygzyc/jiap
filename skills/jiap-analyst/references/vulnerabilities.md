# Vulnerability Patterns

Quick reference for common Android security vulnerabilities.

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
