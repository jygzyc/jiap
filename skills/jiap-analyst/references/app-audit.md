# App Security Audit Workflow

**Scope**: User-space Android Applications (APKs, System Apps)  
**Focus**: IPC vulnerabilities, exported components, data flow analysis, AIDL interfaces, ContentProvider abuse

## Core Principle: Exported Component as Entry Point

**App vulnerabilities always start from exported components.** Before analyzing any code, first identify which components are exported and reachable by attackers.

### Entry Point Verification Framework

| Component Type | Entry Point Method | How to Verify Export |
|----------------|-------------------|---------------------|
| **Activity** | `getIntent()` in `onCreate()` / `onNewIntent()` | Check `android:exported="true"` in manifest |
| **Service** | AIDL interface methods | Check `android:exported="true"` OR implicit intent-filters |
| **ContentProvider** | `query()`, `insert()`, `update()`, `delete()`, `call()` | Check `android:exported="true"` OR permission enforcement |
| **BroadcastReceiver** | `onReceive()` | Check exported flag OR dynamically registered without permission |

**CRITICAL: If component is NOT exported → STOP analysis, it's not attacker-reachable**

---

## Section 1: Activity Analysis - Intent Data Flow

### Step 1.1: Identify Exported Activities

```bash
# Get all exported activities
get_exported_components(1)

# Search for Activity classes with intent handling
search_method("onCreate", 1)
search_method("onNewIntent", 1)

# Get all deep links (alternative entry points)
get_deep_links(1)
```

### Step 1.2: Extract Intent Data (Source Identification)

**The data received via `getIntent()` is the SOURCE of taint.**

```bash
# Find where intent data is extracted
search_method("getIntent", 1)
search_method("getStringExtra", 1)
search_method("getParcelableExtra", 1)
search_method("getExtras", 1)

# For deep links
search_method("getData", 1)
search_method("getQueryParameter", 1)
search_method("getPathSegments", 1)
```

**Source Validation Checklist:**
- [ ] Activity is exported (`android:exported="true"`)
- [ ] OR Activity has intent-filter with data scheme (deep link)
- [ ] Data is extracted via `getIntent()` or deep link methods
- [ ] Data flows TO internal components or sensitive operations

### Step 1.3: Taint Propagation via Cross-Reference

**Use `get_method_xref()` to trace data flow from source to sink.**

```bash
# Example workflow:
# 1. Find Activity receiving intent
get_class_info("com.app.ExportedActivity", 1)

# 2. Read the onCreate method to see intent handling
get_method_source("com.app.ExportedActivity.onCreate(android.os.Bundle):void")
# Returns: Intent intent = getIntent();
#          String userInput = intent.getStringExtra("data");

# 3. Cross-reference where userInput is used
get_method_xref("com.app.ExportedActivity.onCreate(android.os.Bundle):void", 1)
# Shows: Activity calls Helper.processData(userInput)

# 4. Check the helper method
get_method_source("com.app.Helper.processData(java.lang.String):void")
# Shows: processData(String input) { startActivity(new Intent(input)); }

# 5. Verify if there's sanitizer
search_class_key("ALLOWED_INTENTS")  # Empty = no sanitizer

# Result: getIntent() → processData → startActivity (NO sanitizer) = VULNERABLE
```

**Propagation Patterns to Track:**
- Intent stored in field: `this.mIntent = getIntent();`
- Intent forwarded: `newIntent.putExtra("key", getIntent().getStringExtra("data"));`
- Data passed to helper: `processData(getIntent().getExtras());`
- String concatenation: `"action://" + getIntent().getData();`

---

## Section 2: Service Analysis - AIDL Interface Implementation

### Step 2.1: Identify Exported Services

```bash
# Get exported services
get_exported_components(1)

# Search for Service classes
search_class_key("extends Service")
search_method("onBind", 1)
```

### Step 2.2: Analyze AIDL Interface

**AIDL services expose methods through Binder interfaces. Each method is a potential entry point.**

```bash
# Find AIDL interface definitions
search_class_key("extends IInterface")
search_class_key("Stub")

# Find AIDL implementation classes
search_class_key("extends IInterface.Stub")
search_class_key("extends Binder")

# Get service binding
get_method_source("com.app.MyService.onBind(android.content.Intent):android.os.IBinder")
# Returns: return new IMyInterface.Stub() { ... };
```

**AIDL Analysis Workflow:**

```bash
# 1. Find AIDL interface methods
get_class_info("com.app.IMyInterface", 1)
# Shows all methods defined in AIDL

# 2. Read AIDL implementation
get_method_source("com.app.MyService$Stub.processRequest(java.lang.String):void")
# Shows: public void processRequest(String data) { ... }

# 3. Cross-reference to trace data flow
get_method_xref("com.app.MyService$Stub.processRequest(java.lang.String):void", 1)
# Shows which internal methods are called with the AIDL parameter

# 4. Check if data reaches dangerous sinks
search_method("execSQL", 1)
search_method("Runtime.getRuntime", 1)
```

**AIDL Vulnerability Patterns:**
- **Intent Redirection via AIDL**: AIDL method receives Intent, forwards to `startActivity()`
- **Command Injection**: AIDL method executes system command with user input
- **SQL Injection**: AIDL method performs database operation without parameterization
- **File Access**: AIDL method reads/writes files based on user-provided path

---

## Section 3: ContentProvider Analysis - call() and File Operations

### Step 3.1: Identify Exported ContentProviders

```bash
# Get exported content providers
get_exported_components(1)

# Search for ContentProvider implementations
search_class_key("extends ContentProvider")
```

### Step 3.2: Analyze call() Function

**The `call()` method is a hidden entry point that can execute arbitrary code.**

```bash
# Find call() implementations
search_method("call", 1)

# Read call() implementation
get_method_source("com.app.MyProvider.call(java.lang.String,java.lang.String,android.os.Bundle):android.os.Bundle")
# Shows: public Bundle call(String method, String arg, Bundle extras) { ... }
```

**call() Analysis Checklist:**
- [ ] Method parameter flows to dangerous operation
- [ ] Argument parameter used in file path, SQL, or command
- [ ] extras Bundle is deserialized and used without validation
- [ ] No permission check before executing sensitive operation

### Step 3.3: File Read/Write Analysis

**ContentProviders often expose file operations through `openFile()` or custom methods.**

```bash
# Find file operation methods
search_method("openFile", 1)
search_method("openFileHelper", 1)
search_method("openAssetFile", 1)

# Path traversal sinks in ContentProvider
search_class_key("new File")
search_class_key("FileInputStream")
search_class_key("FileOutputStream")
```

**File Operation Analysis Workflow:**

```bash
# 1. Read ContentProvider's file handling
get_method_source("com.app.FileProvider.openFile(android.net.Uri,java.lang.String):android.os.ParcelFileDescriptor")
# Shows: File file = new File(getContext().getFilesDir(), uri.getLastPathSegment());

# 2. Check for path sanitization
search_class_key("getCanonicalPath")  # Should exist for safe file access
search_class_key("path.startsWith")   # Directory restriction check

# 3. Verify if attacker controls file path
search_method("getLastPathSegment", 1)
search_method("getPathSegments", 1)
```

**ContentProvider Vulnerability Patterns:**
- **Path Traversal**: `openFile()` uses user-controlled URI without canonicalization
- **SQL Injection in query()**: URI path used directly in SQL query
- **Arbitrary Code Execution via call()**: Method name maps to internal operations
- **Privilege Escalation**: Provider accesses protected data without permission checks

---

## Core Principle: Taint-Based Verification

---

## Taint Analysis Methodology

### Step 1: Identify Sources (Entry Points)

Find where attacker-controlled data enters the app:

```bash
# Intent-based sources
search_method("getIntent", 1)
search_method("getStringExtra", 1)
search_method("getParcelableExtra", 1)
search_method("getExtras", 1)

# Deep link sources
search_method("getData", 1)
search_method("getQueryParameter", 1)
search_method("getPathSegments", 1)

# Content provider sources
search_method("query", 1)
search_method("insert", 1)
search_method("update", 1)

# Check if sources are actually reachable
get_exported_components(1)
get_deep_links(1)
get_dynamic_receivers(1)
```

**Source is valid ONLY IF:**
- Component is exported (`android:exported="true"`)
- OR deep link is registered (intent-filter with data scheme)
- OR broadcast receiver is dynamically registered without permission

### Step 2: Trace Taint Propagation

Follow the data flow from source to potential sinks:

```bash
# Find methods that process tainted data
get_method_xref("startActivity", 1)           # Intent redirection sinks
get_method_xref("startService", 1)
get_method_xref("sendBroadcast", 1)

# Examine method implementations
get_method_source("com.app.Activity.onCreate(android.os.Bundle):void")
get_method_source("com.app.Helper.processData(java.lang.String):void")

# Search for variable usage in source code
search_class_key("userInput")                   # Search for variable name
search_class_key("getIntent")                   # Find where intent is used

# Check field propagation (taint stored in fields)
search_class_key("this.input")                  
search_class_key("mUserData")
```

**Propagation Patterns to Track:**
- Direct assignment: `String x = source;`
- Field storage: `this.field = source;`
- Collection storage: `list.add(source);`
- Intent forwarding: `newIntent.putExtra("key", source);`
- String concatenation: `"prefix" + source`

### Step 3: Identify Sanitizers

Check if taint is properly sanitized before reaching sink:

| Sanitizer Type | Pattern | Verification Command |
|----------------|---------|---------------------|
| **Allowlist** | `if (ALLOWED.contains(input))` | `search_class_key("ALLOWED")` |
| **Exact Match** | `if ("expected".equals(input))` | `search_class_key("equals")` |
| **Path Canonicalization** | `file.getCanonicalPath().startsWith(base)` | `search_method("getCanonicalPath", 1)` |
| **Type Conversion** | `Integer.parseInt(input)` | Check if conversion is safe |
| **SQL Parameterization** | `rawQuery("...?", args)` | `search_class_key("Parameterized")` |
| **Intent Locking** | `intent.setComponent(knownSafe)` | `search_method("setComponent", 1)` |

**Incomplete Sanitization = No Sanitization:**
```java
// ❌ NOT sufficient - partial check
if (!input.contains("../")) {
    new File(base, input);  // Still vulnerable to other traversal patterns
}

// ✅ Proper sanitization
File f = new File(base, input);
if (!f.getCanonicalPath().startsWith(base.getCanonicalPath())) {
    throw new SecurityException();
}
```

### Step 4: Identify Sinks

Find dangerous execution points where tainted data ends:

```bash
# Component launch sinks
search_method("startActivity", 1)
search_method("startService", 1)
search_method("sendBroadcast", 1)

# Command execution sinks
search_method("Runtime.getRuntime", 1)
search_method("exec", 1)
search_method("ProcessBuilder", 1)

# SQL injection sinks
search_method("rawQuery", 1)
search_method("execSQL", 1)

# Path traversal sinks
search_method("FileInputStream", 1)
search_method("FileOutputStream", 1)
search_method("openFileOutput", 1)

# WebView sinks
search_method("loadUrl", 1)
search_method("loadData", 1)
search_method("evaluateJavascript", 1)

# Reflection sinks
search_method("Class.forName", 1)
search_method("getMethod", 1)
search_method("invoke", 1)
```

### Step 5: Connect Source → Sink

Use xref analysis to build the complete taint path:

```bash
# Example: Intent Redirection
# 1. Find source
search_method("getParcelableExtra", 1)
# Returns: ActivityA.onCreate() calls getParcelableExtra("forward")

# 2. Trace from source method to sink
get_method_xref("com.app.ActivityA.onCreate(android.os.Bundle):void", 1)
# Shows: ActivityA calls Helper.forwardIntent()

# 3. Check helper method
get_method_source("com.app.Helper.forwardIntent(android.content.Intent):void")
# Shows: forwardIntent() calls startActivity()

# 4. Verify no sanitizer
search_class_key("ALLOWED_COMPONENTS")  # No results = no sanitizer

# 5. Confirm sink
get_method_xref("startActivity", 1)  # Verify the call exists

# Result: Source → forwardIntent → Sink without sanitizer = VULNERABLE
```

---

## Reporting Template

```
[SEVERITY] Finding Title
Risk: HIGH/MEDIUM/LOW
Component: class.method()
Cause: Brief description
Evidence:
  // Code snippet
Exploit Path:
  1. Attacker does X
  2. Input reaches Y
  3. Triggers Z
Impact: Security breach
Mitigation: Fix
```
